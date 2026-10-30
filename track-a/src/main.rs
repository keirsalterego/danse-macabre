//! # Track A: Ingest
//!
//! This program reads an access log of about 3.1 million lines and writes, for every hour and
//! endpoint that saw traffic, the request count, the exact 50th, 95th and 99th latency
//! percentiles, and the error rate. An input line looks like this:
//!
//! ```text
//! 2026-10-30T18:07:12.345678Z GET /v2/orders/{id} 200 1234
//! ```
//!
//! and an output line like this (count, p50, p95, p99, errors, errors per million):
//!
//! ```text
//! 2026-10-30T18:00:00Z /v2/orders/{id} 3 5678 9012 9012 0 0
//! ```
//!
//! ## The pipeline
//!
//! One team of threads, one per hardware thread (12 on the scoring box), runs three parallel
//! phases with short serial steps between them ([`summarize`]).
//!
//! 1. **Map and split.** We `mmap` the log instead of reading it ([`map_file`]) and cut it into
//!    one chunk per thread at line breaks ([`summarize`]).
//! 2. **Parse once.** Each thread turns every line of its chunk into a 4-byte record: endpoint
//!    id, error bit and latency, with the rare latency too large for its 19 bits kept on a side
//!    list ([`parse`]). Endpoint names become small integer ids through a per-thread hash table
//!    ([`Interner`]). The hour is not stored per line: the records stay in file order, where
//!    lines come in long runs of one hour, and the thread notes where each run starts
//!    ([`Chunk`]).
//! 3. **Merge.** Each thread adds its names to one shared table, which gives every distinct
//!    endpoint a global id ([`Interner::merge`]). One thread then sorts the 1,842 names
//!    bytewise. That order is also the order of the endpoints within an hour of the output, so
//!    the output never needs sorting.
//! 4. **Summarize by hour.** Threads take hours one at a time, largest first
//!    ([`summarize_hour`]). For one hour a thread goes through that hour's runs in every chunk.
//!    It counts the lines per endpoint, uses the counts to place each latency next to the others
//!    of its endpoint (a counting sort), and picks the three percentiles out of each endpoint's
//!    latencies ([`percentiles`]). What it keeps is six numbers per output line, and the hour's
//!    exact length once formatted. The hour's records are freed as soon as they are read.
//! 5. **Write in place.** With every hour's length known, we give the output file its final size
//!    and map it into memory ([`map_output`]). Each thread then frees what is left of its
//!    chunk's records and formats hours straight into place in the file ([`format_hour`]).
//!
//! ## Why it is fast
//!
//! - **One pass over the input.** The log is 239 MB, ten times the L3 cache, so every pass over
//!   it streams from main memory. We parse it once and keep 12 MB of compact records instead.
//! - **No byte-by-byte scanning.** One AVX2 compare finds a line's end ([`line_end`]). The
//!   latency, the status and the end of the endpoint are then read backwards from it at known
//!   offsets, and the latency's digits convert in a single SWAR step ([`latency`]).
//! - **Working sets that fit in cache.** One hour holds about 43,000 lines, so its latency
//!   buffer (170 KB) stays in the core's L2 cache during the counting sort and the selection.
//! - **No teardown.** We exit right after the write, without running destructors, and the threads
//!   hand their records and their share of the input mapping back to the kernel themselves, in
//!   parallel ([`release`]), instead of leaving it all to the exit.
//!
//! Every step's measurements are in `PERF.md`.

use std::sync::{Barrier, Mutex, OnceLock, atomic::AtomicUsize, atomic::Ordering::Relaxed};
use std::{fs::File, io::Write, ops::Range, os::fd::AsRawFd, path::Path, ptr::null_mut, thread};

/// The number of hours a log can span.
///
/// The spec guarantees that every timestamp lies in the same 72 hours, from
/// `2026-10-30T18:00:00.000000Z` to `2026-11-02T17:59:59.999999Z`, for every seed and scale
/// (§2, field `ts`). That lets us use the hour as a dense index `0..72` instead of hashing it.
const HOURS: usize = 72;

/// The four dates those hours fall on, as a timestamp writes them in its bytes `5..10`.
const DAYS: [&str; 4] = ["10-30", "10-31", "11-01", "11-02"];

/// The length of the shortest possible line, LF included.
///
/// A 27-byte timestamp, a 3-byte method, a 1-byte endpoint, a 3-digit status, a 1-digit latency,
/// four spaces and the LF add up to 40. Two things rely on it: [`parse`] sizes its record buffer
/// for the worst case of all-minimal lines, and [`line_end`] starts looking for the LF at byte 39.
const MIN_LINE: usize = 40;

/// How many bytes from a line's start [`parse`] may read: every line fits, with room to spare.
///
/// The longest line is 177 bytes: a 27-byte timestamp, a 7-byte method, a 125-byte endpoint, a
/// 3-digit status, a 10-digit latency, four spaces and the LF. The parse views each line as
/// a fixed array of this size, so its reads at offsets computed from the line need no checks
/// of their own; [`map_file`] makes sure the array exists even for the last line.
const LINE: usize = 256;

// We declare `mmap` and friends ourselves instead of pulling in the `libc` crate: every
// dependency costs points on the D axis, and the C library is already linked into every Rust
// binary. The flag values are Linux's, from `<sys/mman.h>` and `<sys/prctl.h>`; a comment at
// each call names them.
unsafe extern "C" {
    fn prctl(option: i32, ...) -> i32;
    fn madvise(addr: *const u8, len: usize, advice: i32) -> i32;
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
}

/// Reads `<data_dir>/requests.log` and writes `<out_dir>/summary.txt`.
///
/// The directories default to `/data` and `/out`, where the scoring harness mounts them; the
/// local test scripts pass their own.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A system that enables transparent huge pages for every process (`always`, as on our test
    // machine and, judging by its memory numbers, on the scoring box) backs anonymous memory in
    // 2 MB pages. A thread's 2 MB of records then usually straddles two of them, and that padding
    // was 13 of the 50 MB peak. With plain 4 KB pages the records cost what they hold; the extra
    // page faults did not change the time measurably. The call is advice, so failure is ignored.
    // SAFETY: PR_SET_THP_DISABLE (41) only changes how the kernel backs our future memory.
    unsafe { prctl(41, 1u64, 0u64, 0u64, 0u64) };
    let arg = |n, default: &str| std::env::args().nth(n).unwrap_or(default.into());
    let log = map_file(&Path::new(&arg(1, "/data")).join("requests.log"))?;
    summarize(log, &Path::new(&arg(2, "/out")).join("summary.txt"));
    // Exit without running destructors: freeing the interners and the hours' statistics would
    // only add to the timed wall clock, and the kernel reclaims the memory anyway. Every byte of
    // the output is already in the file's shared mapping, which the kernel keeps.
    std::process::exit(0)
}

/// Creates the file at `path` with `len` zero bytes and maps it shared and writable.
///
/// A single `write` of the 10 MB output took 3.9 ms, all on one thread: the kernel allocates a
/// page-cache page for every 4 KB and copies into it. Several threads calling `write` on one
/// file would queue on its inode lock. Instead we set the file's final size up front and map
/// it, and every thread formats its hours straight into place. The page faults that allocate
/// the pages then happen on all threads at once. Stores through a shared mapping land in the
/// same page cache that `write` fills, so the file's bytes are identical.
fn map_output(path: &Path, len: usize) -> Result<&'static mut [u8], Box<dyn std::error::Error>> {
    // `create` empties the file. Mapping it for writing needs it open for reading too.
    File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file = File::options().read(true).write(true).open(path)?;
    file.set_len(u64::try_from(len)?)?;
    let ptr = map(null_mut(), len, 3, 1, file.as_raw_fd())?; // PROT_READ | PROT_WRITE, MAP_SHARED
    // SAFETY: the mapping holds `len` writable bytes, nothing else refers to them, and it is
    // never unmapped. It stays valid after `file` is closed.
    Ok(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// Maps the whole file into our address space, read-only, followed by [`LINE`] zero bytes.
///
/// Reading the file with `read` would copy all 239 MB into heap memory that counts toward our
/// memory score. A private read-only mapping instead shares the pages the kernel already holds
/// in its page cache (the harness pre-warms it), so the file costs neither a copy nor memory.
/// Pages are mapped lazily as the parser first touches them.
///
/// The zeros come from a reservation of anonymous memory that the file is then mapped over:
/// they let [`parse`] read a whole [`LINE`] from the start of the last line too. Reading them
/// maps the kernel's shared zero page, so they cost no memory either.
fn map_file(path: &Path) -> Result<&'static [u8], Box<dyn std::error::Error>> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let len = usize::try_from(file.metadata()?.len())?;
    let ptr = map(null_mut(), len + LINE, 1, 0x22, -1)?; // PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS
    map(ptr, len, 1, 0x12, file.as_raw_fd())?; // PROT_READ, MAP_PRIVATE | MAP_FIXED
    // SAFETY: the mappings hold `len + LINE` readable bytes and are never unmapped, so they
    // outlive every borrow. /data is mounted read-only, so the bytes cannot change under us.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len + LINE) })
}

/// Maps `len` bytes of the file `fd` (or of anonymous memory, for `fd` -1) from its start with
/// `mmap(2)` at `addr`, or wherever the kernel picks for a null `addr`, and returns the address.
fn map(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32) -> std::io::Result<*mut u8> {
    // SAFETY: a new mapping at an address the kernel picks overlaps no memory we already use, and
    // the one fixed address we pass is our own fresh reservation, which nothing refers to yet.
    // `mmap` rejects a zero length. An empty file or output maps one byte, which nothing reads.
    let ptr = unsafe { mmap(addr, len.max(1), prot, flags, fd, 0) };
    let ok = ptr.addr() != usize::MAX; // on failure `mmap` returns `MAP_FAILED`, the address -1
    ok.then_some(ptr).ok_or_else(std::io::Error::last_os_error)
}

/// Runs the whole pipeline over `log` and writes the output to `out_path`.
///
/// One team of threads runs every parallel phase, in one [`std::thread::scope`], so the threads
/// can borrow `log` and each other's results without reference counting. Between the phases
/// they wait at a barrier, and the first one past it does the short serial step while the
/// others wait for its result in a [`OnceLock`]: ranking the endpoints, and later mapping the
/// output file. Spawning a second team cost the time until its last thread started, about
/// 0.25 ms. A worker that panics aborts the whole process (`panic = "abort"` in `Cargo.toml`)
/// with its message, so the others never wait forever for it at a barrier. That is how the
/// rare errors a worker can meet end the run: failing to create the output file, or input
/// that breaks a guarantee of the spec, which we otherwise rely on without checking.
fn summarize(log: &[u8], out_path: &Path) {
    let threads = thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let len = log.len() - LINE;
    // Chunk t starts at the first line that starts at or after byte `len * t / threads`, so
    // every line lands in exactly one chunk. With fewer lines than threads, some chunks are
    // simply empty.
    let start = |t: usize| {
        let newline = log[..len * t / threads].iter().rposition(|&b| b == b'\n');
        newline.map_or(0, |p| p + 1)
    };
    // Each thread's chunk, with the table from its endpoint ids to ids in `all`, the merge of
    // every chunk's names; then each hour's output lines.
    let chunks: Vec<OnceLock<(Chunk, Vec<u32>)>> = (0..threads).map(|_| OnceLock::new()).collect();
    let hours: Vec<OnceLock<Hour>> = (0..HOURS).map(|_| OnceLock::new()).collect();
    let all = Mutex::new(Interner::default());
    let (plan, places, barrier) = (OnceLock::new(), OnceLock::new(), Barrier::new(threads));
    let (taken, finished) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let work = |t: usize| {
        let (endpoints, chunk) = parse(log, start(t)..start(t + 1));
        // Each thread merges its names as soon as it is done, while others may still parse, and
        // then frees its table: only `all` needs the names from now on.
        let ids = all.lock().expect("panics abort").merge(&endpoints);
        drop(endpoints);
        let _ = chunks[t].set((chunk, ids));
        barrier.wait();
        let (names, order, runs, by_size) = plan.get_or_init(|| {
            // A name's padded copy sorts like the name itself, because the space after it and
            // the zero padding are smaller than any byte a name can hold (§2): a proper prefix
            // sorts first, which is exactly the order the spec asks for (§4). Comparing the
            // copies saves reading 1,842 names from all over the input; only names that agree
            // on 64 bytes compare in full.
            let all = std::mem::take(&mut *all.lock().expect("panics abort"));
            let mut order: Vec<usize> = (0..all.names.len()).collect();
            order.sort_unstable_by_key(|&id| (&all.padded[id].0, all.names[id].0));
            let mut runs: Vec<Vec<Run>> = vec![Vec::new(); HOURS];
            for (chunk, ids) in chunks.iter().map(|c| c.get().expect("parsed")) {
                for &[(hour, start, first), (_, end, _)] in chunk.runs.array_windows() {
                    runs[hour].push((&chunk.records[start..end], &chunk.large[first..], ids));
                }
            }
            // Night hours carry about half the traffic of day hours, so fixed blocks of hours
            // would leave some workers idle while others finish. Workers instead take one hour
            // at a time from a shared counter, largest first (longest-processing-time-first
            // scheduling).
            let mut by_size: Vec<usize> = (0..HOURS).collect();
            let size = |h: &usize| runs[*h].iter().map(|run| run.0.len()).sum::<usize>();
            by_size.sort_by_key(|h| std::cmp::Reverse(size(h)));
            let names: Vec<&[u8]> = all.names.iter().map(|&(name, _)| name).collect();
            (names, order, runs, by_size)
        });
        let mut latencies = Vec::new();
        for &hour in std::iter::from_fn(|| by_size.get(taken.fetch_add(1, Relaxed))) {
            let _ = hours[hour].set(summarize_hour(&runs[hour], order, names, &mut latencies));
        }
        // The output will need this memory soon: every page given back now lowers the peak.
        release(&latencies);
        barrier.wait();
        // Every hour's length is known now, so the first thread here gives each its place.
        let queue = places.get_or_init(|| {
            let hours: Vec<&Hour> = hours.iter().map(|h| h.get().expect("summarized")).collect();
            let len = hours.iter().map(|(_, len)| len).sum();
            let mut rest = map_output(out_path, len).unwrap_or_else(|e| panic!("{e}"));
            let mut place = |&(_, len): &Hour| rest.split_off_mut(..len).expect("lengths add up");
            let places = hours.into_iter().map(|hour| (hour, place(hour)));
            Mutex::new(places.enumerate().collect::<Vec<_>>())
        });
        // The threads format in parallel, taking the next hour from the queue until none is
        // left. Each first frees the rest of its chunk's records, the pages that two runs
        // share, which `summarize_hour` had to keep: no hour needs them any more. Releasing
        // the input waits until the end, because freeing pages interrupts every other thread to
        // flush its TLB, which cost 3 ms in the hour pass.
        let (chunk, _) = chunks[t].get().expect("parsed");
        release(&chunk.records);
        release(&chunk.large);
        let next = || queue.lock().expect("panics abort").pop();
        while let Some((index, ((lines, _), place))) = next() {
            format_hour(index, lines, names, place);
        }
        release(&log[len * t / threads..len * (t + 1) / threads]);
        // The output is complete once the last thread gets here: skip joining the others.
        (finished.fetch_add(1, Relaxed) + 1 == threads).then(|| std::process::exit(0));
    };
    thread::scope(|s| fan_out(s, 0..threads, &work));
}

/// Runs `work(t)` for every `t` in `ts`, each on its own thread, the first on this one.
///
/// Each thread hands the upper half of its numbers to a new thread until it keeps just one, so
/// the last of twelve threads starts after four spawns instead of eleven. A spawn took about
/// 50 µs, and every thread parses an equal share, so the last to start was the last to finish.
fn fan_out<'s, F: Fn(usize) + Sync>(s: &'s thread::Scope<'s, '_>, ts: Range<usize>, work: &'s F) {
    if ts.len() > 1 {
        let mid = ts.start + ts.len() / 2;
        s.spawn(move || fan_out(s, mid..ts.end, work));
        return fan_out(s, ts.start..mid, work);
    }
    work(ts.start);
}

/// Gives the memory pages that lie entirely inside `items` back to the kernel, with
/// `madvise(MADV_DONTNEED)`.
///
/// At exit the kernel frees every page we touched, on one thread and inside the timed window:
/// for the 58,000 pages of the input alone that took 3.9 ms. So each thread frees its share of
/// the input and of the records itself, in parallel with the others, once nothing reads them.
/// Freeing memory early also lowers the peak our memory score counts.
/// A page that only partly belongs to `items` stays mapped, since its other part may still be
/// in use; if there is no whole page, the length is 0 and the call does nothing. The call is
/// advice: if it fails, the exit does the work as before, so the result is ignored.
fn release<T>(items: &[T]) {
    let first = items.as_ptr().map_addr(|a| a.next_multiple_of(4096));
    let end = (items.as_ptr().addr() + size_of_val(items)) & !4095;
    // SAFETY: the pages lie inside `items`. In the input's private read-only file mapping, a
    // later read (an endpoint name, say) just maps the same file data again. Records and
    // latencies read as zeros afterwards, which is why each is released only once nothing
    // reads it again: a run's records once its hour has been placed, a buffer after its last
    // hour.
    unsafe { madvise(first.cast(), end.saturating_sub(first.addr()), 4) }; // 4 is MADV_DONTNEED
}

/// One thread's share of the log after parsing.
///
/// Every line becomes one `u32` record, packed like this:
///
/// | bits    | field                                                  |
/// |---------|--------------------------------------------------------|
/// | 31 – 20 | the endpoint's id in this chunk (12 bits)              |
/// | 19      | 1 if the status is 500–599                             |
/// | 18 – 0  | the latency in microseconds, or [`LARGE`]              |
///
/// The spec allows any 32-bit latency, but 97.5% of seed 1's lines took under half a second,
/// which fits in 19 bits. A latency of [`LARGE`] or more goes to the `large` list instead, in
/// line order, and its record holds [`LARGE`]: whoever reads the records in order takes the
/// next value from `large` each time they meet it.
///
/// The hour is not stored per line. Shards upload their lines in time order, so consecutive
/// lines of the log almost always share an hour: seed 1's 3.1 million lines form just 1,054 runs
/// of one hour each. The chunk keeps one entry per run instead, and the records stay in file
/// order without any sorting. Correctness does not depend on long runs, since the spec promises
/// no bound on the disorder: a log whose hour changed on every line would simply have one run
/// per line.
///
/// Four bytes per line keep all 3.1 million records at 12.4 MB. The ids are local to the chunk:
/// [`Interner::merge`] maps them to global ids.
#[derive(Default)]
struct Chunk {
    records: Vec<u32>,
    large: Vec<u32>,
    /// Where each run starts, in file order: its hour, its first record, and the index in
    /// `large` of its first large latency. A last entry, with hour [`HOURS`], marks the end.
    runs: Vec<(usize, usize, usize)>,
}

/// The value of a record's 19-bit latency field that sends readers to the chunk's list of large
/// latencies (see [`Chunk`]).
const LARGE: u32 = (1 << 19) - 1;

/// Parses one chunk of whole lines into records, and returns them with the chunk's endpoints.
///
/// The spec guarantees well-formed input (§2), so we locate fields by fixed offsets and single
/// byte checks instead of validating them. No field is scanned byte by byte. Once [`line_end`]
/// has found the LF, everything else sits at a known offset from either end of the line:
///
/// | field     | position                                                         |
/// |-----------|------------------------------------------------------------------|
/// | timestamp | bytes `0..27`: month at 5, day at 8, hour at 11                  |
/// | method    | 3 to 7 bytes from byte 28                                        |
/// | endpoint  | from the byte after the method's space up to `space - 4`         |
/// | status    | the three bytes `space - 3 .. space`                             |
/// | latency   | the digits between `space` and the LF                            |
///
/// Here `space` is the last space of the line, which [`latency`] finds.
fn parse(log: &[u8], lines: Range<usize>) -> (Interner<'_>, Chunk) {
    let (mut endpoints, mut chunk) = (Interner::default(), Chunk::default());
    // Every line takes at least MIN_LINE bytes. The records start out zero, which costs nothing
    // until a page is written, and the unused tail is cut off at the end.
    chunk.records = vec![0; lines.len() / MIN_LINE + 1];
    // A run lasts while the timestamp's bytes 5 to 12, `MM-DDTHH`, stay the same, so most lines
    // compare those 8 bytes and decode nothing. No line's bytes are all zero, so line 0 starts a
    // run.
    let (mut count, mut current, mut i) = (0, 0, lines.start);
    assert!(lines.end + LINE <= log.len(), "map_file pads the log");
    while i < lines.end {
        // SAFETY: i < lines.end, so the LINE bytes from i lie inside `log` (asserted above).
        let line: &[u8; LINE] = unsafe { &*log.as_ptr().add(i).cast() };
        let lf = line_end(line);
        i += lf + 1;
        if word(line, 5) != current {
            let hour = hour_index(line).expect("timestamps lie in the 72 hours (§2)");
            chunk.runs.push((hour, count, chunk.large.len()));
            current = word(line, 5);
        }
        // The method is 3 to 7 bytes at offset 28, so the space after it is in the word at 31.
        // It is the first byte there without bit 6 (0x40), which every capital letter has.
        let start = 32 + (!word(line, 31) & 0x4040_4040_4040_4040).trailing_zeros() as usize / 8;
        // `space` precedes the latency; before it sit the 3-digit status and its own space.
        let (latency_us, space) = latency(line, lf);
        let name = &line[start..space - 4];
        let window = line[start..start + 64].try_into().expect("start <= 40");
        let id = endpoints.intern(name, hash(name, word(line, start)), window);
        // Errors are exactly 500..=599, so the status's first digit decides.
        let error = u32::from(line[space - 3] == b'5');
        if latency_us >= LARGE {
            chunk.large.push(latency_us);
        }
        chunk.records[count] = id << 20 | error << 19 | latency_us.min(LARGE);
        count += 1;
    }
    chunk.records.truncate(count);
    chunk.runs.push((HOURS, count, chunk.large.len()));
    (endpoints, chunk)
}

/// Returns the index of the LF that ends the first line of `line`.
///
/// No line is shorter than [`MIN_LINE`], so the search starts at byte 39 rather than 0. From
/// there one 64-byte window, checked with two AVX2 compares in [`equal_mask`], holds the LF of
/// about 90% of lines, which makes the branch after it easy to predict. Longer lines take
/// another window; three reach byte 230, well past the LF of the longest line.
fn line_end(line: &[u8; LINE]) -> usize {
    for at in [MIN_LINE - 1, MIN_LINE + 63, MIN_LINE + 127] {
        // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
        let newlines = unsafe { equal_mask(line[at..].first_chunk().expect("64"), &[b'\n'; 64]) };
        if newlines != 0 {
            return at + newlines.trailing_zeros() as usize;
        }
    }
    panic!("a line is at most 177 bytes long (spec §2)")
}

/// Returns a mask with bit `i` set wherever `a[i] == b[i]`.
///
/// AVX2 compares 32 bytes at once: `_mm256_cmpeq_epi8` turns every matching byte into `0xFF`,
/// and `_mm256_movemask_epi8` gathers the top bit of each byte into a 32-bit integer. Two of
/// those cover the 64-byte blocks. Against a block of LFs, `trailing_zeros` on the result finds
/// the first LF ([`line_end`]); against another name, a mask of all ones means the bytes agree
/// ([`same_name`]).
#[target_feature(enable = "avx2")]
fn equal_mask(a: &[u8; 64], b: &[u8; 64]) -> u64 {
    use std::arch::x86_64::{__m256i, _mm256_cmpeq_epi8 as equal, _mm256_movemask_epi8 as mask};
    // SAFETY: 64 bytes are two 256-bit vectors, and any bits make a valid vector.
    let [a, b]: [[__m256i; 2]; 2] = unsafe { std::mem::transmute([*a, *b]) };
    let low = mask(equal(a[0], b[0])).cast_unsigned();
    u64::from(low) | u64::from(mask(equal(a[1], b[1])).cast_unsigned()) << 32
}

/// Returns the hour index `0..72` of a line's timestamp, or `None` outside the span.
///
/// The timestamp is fixed-width, so we read the month and day, and the hour digits, straight
/// from their offsets instead of parsing a date. The span starts at 18:00 on the first of the
/// [`DAYS`], so the index is `day × 24 + hour − 18`. For example `2026-11-01T00:...` is index
/// 30.
fn hour_index(line: &[u8]) -> Option<usize> {
    let day = DAYS.iter().position(|day| day.as_bytes() == &line[5..10])?;
    let hour = day * 24 + usize::from(line[11] - b'0') * 10 + usize::from(line[12] - b'0');
    hour.checked_sub(18).filter(|&hour| hour < HOURS)
}

/// Returns the latency that ends at `lf`, and the index of the space before it.
///
/// Latencies of up to 7 digits (99.9% of lines) take one step without a loop. We load the 8
/// bytes before the LF as one integer and find the last space in it: digits (`0x30` to `0x39`)
/// have bit 4 set and a space (`0x20`) does not, so it is the highest byte without bit 4. Other
/// bytes without it can only sit further down, in the endpoint. We then keep the low half of
/// every byte above that space, which is its digit, and convert them with [`eight_digits`].
/// For a line ending in `200 1234`, the word holds exactly those 8 bytes, the last space is
/// byte 3, and the digit lanes left are `0 0 0 0 1 2 3 4`.
fn latency(line: &[u8], lf: usize) -> (u32, usize) {
    let w = word(line, lf - 8);
    let found = !w & 0x1010_1010_1010_1010;
    if found == 0 {
        // 8 to 10 digits: rare enough for a plain loop.
        let space = line[..lf].iter().rposition(|&b| b == b' ');
        let space = space.expect("a space precedes the latency");
        let digits = line[space + 1..lf].iter();
        return (digits.fold(0, |v, &d| v * 10 + u32::from(d - b'0')), space);
    }
    let last_space = 7 - found.leading_zeros() as usize / 8;
    let digits = w & 0x0F0F_0F0F_0F0F_0F0F & u64::MAX << (8 * (last_space + 1));
    (eight_digits(digits), lf - 8 + last_space)
}

/// Converts eight decimal digits, one per byte, into their value.
///
/// Each byte holds a digit value `0..=9`, the most significant digit in the lowest byte; unused
/// leading lanes are zero. This is SWAR (SIMD within a register): instead of eight
/// multiply-adds, we combine neighbouring digits into pairs (`d0·10 + d1`), then pairs into the
/// final number, using three multiplies on the whole word. For the lanes `0 0 0 0 1 2 3 4` the
/// pairs are `00 00 12 34`, and the result is `12·100 + 34 = 1234`. The method is from Daniel
/// Lemire, "Fast numeric string to int" (2018).
fn eight_digits(lanes: u64) -> u32 {
    let pairs = lanes * 10 + (lanes >> 8);
    let high = (pairs & 0x0000_00FF_0000_00FF).wrapping_mul(100 + (1_000_000 << 32));
    let low = ((pairs >> 16) & 0x0000_00FF_0000_00FF).wrapping_mul(1 + (10_000 << 32));
    ((high + low) >> 32) as u32
}

/// Reads the 8 bytes at `at` as one little-endian integer, so that byte `at` becomes the lowest
/// byte of the result.
fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(*bytes[at..].first_chunk().expect("8 bytes at `at`"))
}

/// Hashes an endpoint name for the [`Interner`], given `first`, the 8 bytes it starts with.
///
/// Hashing every byte would need a loop whose length varies from line to line. Instead we mix
/// the length with three 8-byte windows: the first, the middle and the last. That is a fixed
/// amount of work, and it covers every byte of names up to 24 bytes long. Each window lies
/// inside the name, so equal names always hash equal. Two different names that agree on all
/// three windows would only share a probe sequence: the interner still compares the bytes.
///
/// The windows are rotated apart and XORed, and one multiply mixes the result, whose top bits
/// then depend on every bit (the interner indexes by them). Three dependent multiply-rotate
/// steps, as in rustc's `FxHasher`, cost 21 million more instructions in the parse and saved
/// few second probes: 3.1% of lines took one, against 3.2% now (seeds 1, 3 and 5, simulated).
fn hash(name: &[u8], first: u64) -> u64 {
    let n = name.len();
    let words = if n < 8 {
        first & u64::MAX >> (64 - 8 * n)
    } else {
        first ^ word(name, n / 2 - 4).rotate_left(21) ^ word(name, n - 8).rotate_left(42)
    };
    (words ^ n as u64).wrapping_mul(0x517c_c1b7_2722_0a95)
}

/// A hash table that gives every distinct endpoint name a dense id: 0, 1, 2, and so on.
///
/// It uses open addressing with linear probing. A name's hash picks a slot, and on a collision
/// we try the next slot until we find the name or an empty slot. The table has 8,192 slots, so
/// with the 1,842 endpoints of a seed it is less than a quarter full: at half full, 7.8% of
/// seed 1's lines needed a second probe, each a mispredicted branch; at a quarter, 3.4% do. It
/// never grows: ids have 12 bits in a record, so a table takes at most 4,096 names, and at that
/// it is half full and probes still end.
struct Interner<'a> {
    /// The table. A slot holds `id + 1`, so that 0 can mark an empty slot. Its size is part of
    /// its type, so a probe needs no bounds check: 0.9% fewer instructions in the parse.
    slots: Box<[u32; 2 << 12]>,
    /// Id → (name, hash). Names borrow the mapped file, so none are copied. The hash lets
    /// [`Interner::merge`] add a chunk's names without reading them from the input again.
    names: Vec<(&'a [u8], u64)>,
    /// Id → the name and the space after it, zero-padded to 64 bytes, for [`same_name`]. A name of
    /// 64 bytes or more has no room for the space and keeps only its first 64 bytes.
    padded: Vec<Padded>,
}

/// A padded copy, aligned to one cache line. The allocator aligns the table to 16 bytes, so
/// each copy used to span two lines, and every probe loaded both: 16% of the L1 misses.
#[repr(align(64))]
struct Padded([u8; 64]);

// The tables never grow: each growing step leaves the old buffer behind in the allocator,
// which kept 0.6 MB more memory in use. Capacity that is never written costs nothing.
impl Default for Interner<'_> {
    fn default() -> Self {
        Self {
            slots: Box::new([0; 2 << 12]),
            names: Vec::with_capacity(1 << 12),
            padded: Vec::with_capacity(1 << 12),
        }
    }
}

// `intern` runs once per line. As a call, with its register saves and bounds checks, it cost
// 9% of all instructions (about 1 ms of wall time), so it is forced inline (`inline_always` is
// allowed in `Cargo.toml`); `insert` stays out of line.
impl<'a> Interner<'a> {
    /// Returns `name`'s id, assigning the next free one if the name is new.
    ///
    /// `window` is 64 bytes that start with `name` and the space after it: the line itself, or a
    /// padded copy. With them a probe compares a name of up to 63 bytes without a call to
    /// `memcmp` (see [`same_name`]). That covers 1,791 of the 1,842 endpoints of seed 1 and 94%
    /// of its lines; longer names compare with `==`. Folding the two into one test, the window
    /// first and `==` only for long names, cost 5% more instructions in the parse.
    #[inline(always)]
    fn intern(&mut self, name: &'a [u8], hash: u64, window: &[u8; 64]) -> u32 {
        // The top 13 bits, one of 8,192 slots: they are the ones that every hashed bit reaches.
        let mut slot = (hash >> 51) as usize;
        while let Some(id) = self.slots[slot].checked_sub(1) {
            let found = if name.len() < 64 {
                same_name(window, &self.padded[id as usize].0, name.len())
            } else {
                self.names[id as usize].0 == name
            };
            if found {
                return id;
            }
            slot = (slot + 1) & (self.slots.len() - 1);
        }
        self.insert(slot, name, window)
    }

    /// Gives `name`, which belongs in the empty `slot`, the next id. New names are rare (one
    /// per distinct endpoint per chunk), so this stays out of line, away from the hot loop. It
    /// hashes the name again instead of taking the hash from `intern`: keeping that alive
    /// through the probe loop cost 5.8 million instructions in the parse.
    #[cold]
    #[inline(never)]
    fn insert(&mut self, slot: usize, name: &'a [u8], window: &[u8; 64]) -> u32 {
        // The spec gives every seed 1,842 endpoints (§2), well below the 4,096 that 12 bits hold.
        assert!(self.names.len() < 1 << 12, "over 4,096 distinct endpoints");
        // The name, then the space if there is room for it, then zeros.
        let mut padded = *window;
        padded[name.len().min(63) + 1..].fill(0);
        self.padded.push(Padded(padded));
        self.names.push((name, hash(name, word(window, 0))));
        self.slots[slot] = self.names.len() as u32;
        self.names.len() as u32 - 1
    }

    /// Adds the names of `chunk`, another table, and returns a table from their ids there to
    /// their ids here.
    ///
    /// It reuses the stored hashes and compares the padded copies, never the names: each name's
    /// bytes in the input lie in a different place, and reading them there took most of a
    /// millisecond, one cache miss after another, while the other threads waited for the lock.
    /// Hashing the names again cost 0.4 ms for that reason.
    fn merge(&mut self, chunk: &Interner<'a>) -> Vec<u32> {
        let names = chunk.names.iter().zip(&chunk.padded);
        (names.map(|(&(name, hash), padded)| self.intern(name, hash, &padded.0))).collect()
    }
}

/// Returns whether the name of `len < 64` bytes that starts `window`, followed by a space, is
/// the name whose padded copy is `known`.
///
/// Names vary in length from line to line, and the branches inside `memcmp` that pick a path
/// by length mispredict often; that was about a quarter of the parse loop. Here the length
/// never steers a branch: we compare all 64 bytes of both blocks at once and keep the result
/// for bytes `0..=len`, the name and the space: `u64::MAX >> (63 - len)` keeps bit `i` exactly
/// for `i <= len`. Comparing the space makes the answer exact without comparing lengths. The
/// spec rules out spaces in endpoints (§2), so a known name longer than `len` has no space at
/// `len`, and one shorter has its space where the window still holds part of the name. The
/// copies are 64 bytes, not the 125 a name can reach, because half the size keeps more of them
/// in cache: that was worth 0.8 ms.
fn same_name(window: &[u8; 64], known: &[u8; 64], len: usize) -> bool {
    // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
    !unsafe { equal_mask(window, known) } & u64::MAX >> (63 - len) == 0
}

/// One hour of output, ready to format: [`summarize_hour`] fills it, [`format_hour`] writes it.
///
/// First its lines, one entry per output line in output order: the endpoint's id, then the
/// request count, the p50, p95 and p99 latencies and the error count. Then the exact length of
/// the formatted hour, in bytes, so that its place in the output file is known before any hour
/// is formatted.
type Hour = (Vec<[u32; 6]>, usize);

/// One run of lines from one hour: their records, their chunk's large latencies from the run's
/// first one on (see [`Chunk`]), and the table from their chunk's endpoint ids to global ids.
type Run<'a> = (&'a [u32], &'a [u32], &'a [u32]);

/// Computes the output lines of one hour from its `runs` in every chunk.
///
/// We go through the runs twice:
///
/// 1. **Count** the lines and errors of each endpoint (by global id).
/// 2. **Place** each latency in one buffer, `buffer`, just below where its endpoint's
///    latencies end. A running sum of the counts tells where that is. Together, the two steps
///    are a counting sort by endpoint.
///
/// Then we take the endpoints in output `order`, and the three percentiles of each. A thread
/// keeps `buffer` from one hour to the next.
///
/// Each run's records are freed right after the second pass, their last reader. Before, all
/// 12 MB of records stayed until the hour pass ended, when the hours' lines and every
/// thread's buffer had joined them: that was the program's memory peak.
fn summarize_hour(runs: &[Run], order: &[usize], names: &[&[u8]], buffer: &mut Vec<u32>) -> Hour {
    // `start[e]` first counts endpoint e's lines, then, after the running sum, holds the end of
    // its latencies. Placing each latency moves it down by one, so it ends as their start.
    let (mut start, mut errors) = (vec![0u32; names.len() + 1], vec![0u32; names.len()]);
    for &(records, _, ids) in runs {
        for &r in records {
            let e = ids[(r >> 20) as usize] as usize;
            start[e] += 1;
            errors[e] += r >> 19 & 1;
        }
    }
    for e in 1..start.len() {
        start[e] += start[e - 1];
    }
    buffer.resize(start[names.len()] as usize, 0);
    for &(records, large, ids) in runs {
        let mut large = large.iter();
        for &r in records {
            let e = ids[(r >> 20) as usize] as usize;
            start[e] -= 1;
            buffer[start[e] as usize] = match r & LARGE {
                LARGE => *large.next().expect("one large latency per LARGE record"),
                small => small,
            };
        }
        release(records);
    }
    let (mut lines, mut len) = (Vec::with_capacity(order.len()), 0);
    // The number of decimal digits `put_number` writes for `value`.
    let digits = |value: u64| value.checked_ilog10().map_or(1, |log| log as usize + 1);
    for &e in order {
        let group = &mut buffer[start[e] as usize..start[e + 1] as usize];
        if !group.is_empty() {
            let [p50, p95, p99] = percentiles(group);
            let line = [e as u32, group.len() as u32, p50, p95, p99, errors[e]];
            // The label and its space, the name, six numbers that each follow a space, and the LF.
            let numbers: usize = numbers(&line).iter().map(|&value| 1 + digits(value)).sum();
            len += 21 + names[e].len() + numbers + 1;
            lines.push(line);
        }
    }
    (lines, len)
}

/// Returns the p50, p95 and p99 of `group`, which it reorders.
///
/// The spec's nearest-rank percentile (§4) is the value at 1-indexed rank ceil(p·n / 100), which
/// it writes as (p·n + 99) / 100; we subtract 1 for a 0-based index. Groups of up to 64 values,
/// 95% of them, go through a [`sort_network`]. Larger ones use `select_nth_unstable(k)`, which
/// puts the k-th smallest value at index k, everything smaller before it and everything larger
/// after it, in linear time. Selecting the median first leaves both higher ranks in the upper
/// half, and p95 then leaves p99 in the top 5%, so the three selections cover about 1.55 n
/// values instead of the 2.94 n of going from p99 down. The suffixes start at the rank already
/// found because the ranks coincide for small groups.
fn percentiles(group: &mut [u32]) -> [u32; 3] {
    let at @ [i50, i95, i99] = [50, 95, 99].map(|p| (p * group.len()).div_ceil(100) - 1);
    match group.len() {
        // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
        ..=8 => unsafe { sort_network::<1>(group, at) },
        // SAFETY: as above.
        9..=16 => unsafe { sort_network::<2>(group, at) },
        // SAFETY: as above.
        17..=32 => unsafe { sort_network::<4>(group, at) },
        // SAFETY: as above.
        33..=64 => unsafe { sort_network::<8>(group, at) },
        _ => {
            let p50 = *group.select_nth_unstable(i50).1;
            let p95 = *group[i50..].select_nth_unstable(i95 - i50).1;
            [p50, p95, *group[i95..].select_nth_unstable(i99 - i95).1]
        }
    }
}

/// Returns the values at the sorted positions `at` of `group`, which holds at most 8·R values,
/// after sorting them in R AVX2 registers.
///
/// Selecting from a group of 4 to 15 values cost about 170 ns, most of it in mispredicted
/// branches, and 80% of the groups are that small. A sorting network compares fixed pairs of
/// positions whatever the values are, so it has no branches at all. This one is bitonic: to
/// merge sorted runs of `k / 2` into runs of `k`, it pairs each position `i` with `i ^ m`, first
/// for `m = k - 1` (its mirror in the run of `k`), then for `m = k / 4, …, 1`, and the lower
/// position of each pair keeps the smaller value. One permute lines up every lane with its
/// partner, which sits in register `r ^ (m / 8)`. Positions past the group hold 0, so they sort
/// first and move the wanted positions up by their number.
#[target_feature(enable = "avx2")]
fn sort_network<const R: usize>(group: &[u32], at: [usize; 3]) -> [u32; 3] {
    use std::arch::x86_64::*; // the twelve AVX2 intrinsics below
    let (lanes, splat) = (_mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7), _mm256_set1_epi32);
    let mut v: [__m256i; R] = std::array::from_fn(|r| {
        // A masked load: lanes past the group's end read nothing and hold 0. Reading lane by
        // lane, with a bounds check on each, cost 1.7% more cycles in all.
        let inside = _mm256_cmpgt_epi32(splat(group.len() as i32 - 8 * r as i32), lanes);
        // SAFETY: the mask keeps the load inside `group`; masked lanes touch no memory.
        unsafe { _mm256_maskload_epi32(group.as_ptr().wrapping_add(8 * r).cast(), inside) }
    });
    let mut k = 2;
    while k <= 8 * R {
        let mut m = k - 1;
        while m > 0 {
            let (old, top) = (v, splat(1 << m.ilog2()));
            for (r, v) in v.iter_mut().enumerate() {
                let to = _mm256_xor_si256(lanes, splat(m as i32 & 7));
                // Each lane's partner `p`. Within a register (m < 8) it comes from `v` itself, not
                // from the copy in `old`, which saves a store and a reload: 0.5% fewer instructions.
                let p = _mm256_permutevar8x32_epi32(if m < 8 { *v } else { old[r ^ (m / 8)] }, to);
                let position = _mm256_add_epi32(lanes, splat(8 * r as i32));
                let upper = _mm256_cmpeq_epi32(_mm256_and_si256(position, top), top);
                let (low, high) = (_mm256_min_epu32(*v, p), _mm256_max_epu32(*v, p));
                *v = _mm256_blendv_epi8(low, high, upper);
            }
            m = if m == k - 1 { k / 4 } else { m / 2 };
        }
        k *= 2;
    }
    // SAFETY: both types are R·32 bytes of plain integers.
    let sorted: [[u32; 8]; R] = unsafe { std::mem::transmute_copy(&v) };
    at.map(|i| sorted.as_flattened()[i + 8 * R - group.len()])
}

/// The six numbers an output line prints after the endpoint's name (spec §4): the request
/// count, p50, p95, p99, the error count and the errors per million requests, rounded down.
fn numbers(&[_, count, p50, p95, p99, errors]: &[u32; 6]) -> [u64; 6] {
    let [count, p50, p95, p99, errors] = [count, p50, p95, p99, errors].map(u64::from);
    [count, p50, p95, p99, errors, errors * 1_000_000 / count]
}

/// Writes the formatted `lines` of hour index `hour` to `place`: the hour's label
/// `YYYY-MM-DDTHH:00:00Z` (the inverse of [`hour_index`]) and a space, the endpoint, and the
/// six numbers.
///
/// Each line is formatted in a small buffer on the stack and then copied into place, because
/// [`put_number`] writes 8 bytes at a time and may write past a line's end. The buffer keeps
/// the label from line to line. A `Vec` instead cost a capacity check and a length update for
/// every number, and a whole hour's buffer per thread kept 1.6 MB more in memory while the
/// output filled up.
fn format_hour(hour: usize, lines: &[[u32; 6]], names: &[&[u8]], mut place: &mut [u8]) {
    let (mut text, day, hh) = ([0; 256], (hour + 18) / 24, (hour + 18) % 24);
    text[..21].copy_from_slice(format!("2026-{}T{hh:02}:00:00Z ", DAYS[day]).as_bytes());
    for line in lines {
        let mut end = 21 + names[line[0] as usize].len();
        text[21..end].copy_from_slice(names[line[0] as usize]);
        for value in numbers(line) {
            end = put_number(&mut text, end, value);
        }
        text[end] = b'\n';
        place.write_all(&text[..=end]).expect("lengths add up"); // fails if the hour's length was short
    }
}

/// Writes a space and then `value` in decimal to `text[at..]`, and returns where they end.
///
/// About 760,000 numbers go through here, so each one takes a fixed sequence of instructions:
/// [`eight_ascii`] turns it into eight digits at once, we shift the leading zeros out to the
/// end, where the next write lands on them, and one 8-byte store writes the space and up to
/// seven digits. The leading zeros are the low bytes that hold `'0'`; the last digit counts
/// even if it is one, so that 0 prints as `0`. The rare value of eight to ten digits writes its
/// leading digits first, then the other seven.
fn put_number(text: &mut [u8], at: usize, value: u64) -> usize {
    if value >= 10_000_000 {
        let at = put_number(text, at, value / 10_000_000);
        text[at..at + 8].copy_from_slice(&(eight_ascii(value % 10_000_000) >> 8).to_le_bytes());
        return at + 7;
    }
    let ascii = eight_ascii(value);
    let zeros = ((ascii ^ 0x3030_3030_3030_3030) | 1 << 56).trailing_zeros() as usize / 8;
    text[at..at + 8].copy_from_slice(&((ascii >> (8 * zeros)) << 8 | 0x20).to_le_bytes());
    at + 9 - zeros
}

/// Returns `value`, which must be below 10^8, as eight ASCII digits with leading zeros, the most
/// significant one in the lowest byte.
///
/// This is SWAR, the reverse of [`eight_digits`]: we split the number into two 4-digit halves
/// in 32-bit lanes, each of those into two 2-digit halves in 16-bit lanes, and those into single
/// digits in bytes. Each split divides every lane at once by multiplying with a reciprocal,
/// `x / 100 = x · 10486 >> 20` and `x / 10 = x · 103 >> 10`, which are exact for these ranges.
fn eight_ascii(value: u64) -> u64 {
    let x = (value / 10_000) | ((value % 10_000) << 32);
    let hundreds = ((x * 10_486) >> 20) & 0x0000_007F_0000_007F;
    let x = hundreds | (x - hundreds * 100) << 16;
    let tens = ((x * 103) >> 10) & 0x000F_000F_000F_000F;
    (tens | (x - tens * 10) << 8) | 0x3030_3030_3030_3030
}
