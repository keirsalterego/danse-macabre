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
//! The work runs in three parallel phases with short serial steps between them. Each parallel
//! phase uses every hardware thread (12 on the scoring box).
//!
//! 1. **Map and split.** We `mmap` the log instead of reading it ([`map_file`]) and cut it into
//!    one chunk per thread at line breaks ([`line_bounds`]).
//! 2. **Parse once.** Each thread turns every line of its chunk into a 4-byte record: endpoint
//!    id, error bit and latency, with the rare latency too large for its 19 bits kept on a side
//!    list ([`parse`]). Endpoint names become small integer ids through a per-thread hash table
//!    ([`Interner`]). The hour is not stored per line: the records stay in file order, where
//!    lines come in long runs of one hour, and the thread notes where each run starts
//!    ([`Chunk`]).
//! 3. **Merge.** We collect every thread's endpoint names, drop duplicates and sort them bytewise
//!    ([`rank_endpoints`]). An endpoint's position in that list, its *rank*, is also its position
//!    within an hour of the output, so the output never needs sorting.
//! 4. **Summarize by hour.** Threads take hours one at a time, largest first
//!    ([`summarize_hours`]). For one hour a thread goes through that hour's runs in every chunk.
//!    It counts the lines per endpoint, uses the counts to place each latency next to the others
//!    of its endpoint (a counting sort), and picks the three percentiles out of each endpoint's
//!    latencies with a selection algorithm, which is linear, instead of a full sort. What it
//!    keeps is six numbers per output line, and the hour's exact length once formatted.
//! 5. **Write in place.** With every hour's length known, we give the output file its final size
//!    and map it into memory ([`map_output`]). Each thread then frees one chunk's records and
//!    formats its run of hours straight into place in the file ([`format_hour`]).
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

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Barrier, Mutex, OnceLock};
use std::{cmp::Reverse, error::Error, fs::File, num::NonZeroUsize, ops::Range, os::fd::AsRawFd};
use std::{path::Path, thread};

/// The number of hours a log can span.
///
/// The spec guarantees that every timestamp lies in the same 72 hours, from
/// `2026-10-30T18:00:00.000000Z` to `2026-11-02T17:59:59.999999Z`, for every seed and scale
/// (§2, field `ts`). That lets us use the hour as a dense index `0..72` instead of hashing it.
const HOURS: usize = 72;

/// The length of the shortest possible line, LF included.
///
/// A 27-byte timestamp, a 3-byte method, a 1-byte endpoint, a 3-digit status, a 1-digit latency,
/// four spaces and the LF add up to 40. Two things rely on it: [`parse`] sizes its record buffer
/// for the worst case of all-minimal lines, and [`line_end`] starts looking for the LF at byte 39.
const MIN_LINE: usize = 40;

// We declare `mmap` and friends ourselves instead of pulling in the `libc` crate: every
// dependency costs points on the D axis, and the C library is already linked into every Rust
// binary. The constants are the values from Linux's `<sys/mman.h>` and `<sys/prctl.h>`.
unsafe extern "C" {
    fn prctl(option: i32, ...) -> i32;
    fn madvise(addr: *const u8, len: usize, advice: i32) -> i32;
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
}
const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const MAP_SHARED: i32 = 1;
const MAP_PRIVATE: i32 = 2;
const MADV_DONTNEED: i32 = 4;
const PR_SET_THP_DISABLE: i32 = 41;

/// Reads `<data_dir>/requests.log` and writes `<out_dir>/summary.txt`.
///
/// The directories default to `/data` and `/out`, where the scoring harness mounts them; the
/// local test scripts pass their own.
fn main() -> Result<(), Box<dyn Error>> {
    // A system that enables transparent huge pages for every process (`always`, as on our test
    // machine and, judging by its memory numbers, on the scoring box) backs anonymous memory in
    // 2 MB pages. A thread's 2 MB of records then usually straddles two of them, and that padding
    // was 13 of the 50 MB peak. With plain 4 KB pages the records cost what they hold; the extra
    // page faults did not change the time measurably. The call is advice, so failure is ignored.
    // SAFETY: PR_SET_THP_DISABLE only changes how the kernel backs our future memory.
    unsafe { prctl(PR_SET_THP_DISABLE, 1u64, 0u64, 0u64, 0u64) };
    let mut args = std::env::args().skip(1);
    let data_dir = args.next().unwrap_or_else(|| "/data".into());
    let out_dir = args.next().unwrap_or_else(|| "/out".into());
    let log = map_file(&Path::new(&data_dir).join("requests.log"))?;
    summarize(log, &Path::new(&out_dir).join("summary.txt"))?;
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
fn map_output(path: &Path, len: usize) -> Result<&'static mut [u8], Box<dyn Error>> {
    // `create` empties the file. Mapping it for writing needs it open for reading too.
    File::create(path)?;
    let file = File::options().read(true).write(true).open(path)?;
    if len == 0 {
        return Ok(&mut []); // mmap rejects a zero length
    }
    file.set_len(u64::try_from(len)?)?;
    let ptr = map(&file, len, PROT_READ | PROT_WRITE, MAP_SHARED)?;
    // SAFETY: the mapping holds `len` writable bytes, nothing else refers to them, and it is
    // never unmapped. It stays valid after `file` is closed.
    Ok(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// Maps the whole file into our address space, read-only.
///
/// Reading the file with `read` would copy all 239 MB into heap memory that counts toward our
/// memory score. A private read-only mapping instead shares the pages the kernel already holds
/// in its page cache (the harness pre-warms it), so the file costs neither a copy nor memory.
/// Pages are mapped lazily as the parser first touches them.
fn map_file(path: &Path) -> Result<&'static [u8], Box<dyn Error>> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let len = usize::try_from(file.metadata()?.len())?;
    if len == 0 {
        return Ok(&[]); // mmap rejects a zero length
    }
    let ptr = map(&file, len, PROT_READ, MAP_PRIVATE)
        .map_err(|e| format!("mmap {}: {e}", path.display()))?;
    // SAFETY: the mapping holds `len` readable bytes and is never unmapped, so it outlives every
    // borrow. /data is mounted read-only, so the bytes cannot change under us.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Maps `len` bytes of `file` from its start with `mmap(2)` and returns the address, which the
/// kernel picks.
fn map(file: &File, len: usize, prot: i32, flags: i32) -> std::io::Result<*mut u8> {
    // SAFETY: a new mapping at an address the kernel picks overlaps no memory we already use.
    let ptr = unsafe { mmap(std::ptr::null_mut(), len, prot, flags, file.as_raw_fd(), 0) };
    if ptr.addr() == usize::MAX {
        return Err(std::io::Error::last_os_error());
    }
    Ok(ptr)
}

/// Runs the whole pipeline over `log` and writes the output to `out_path`.
///
/// The parallel phases run in two [`std::thread::scope`]s, one for the parse and one for the
/// hour pass and the formatting, so the threads can borrow `log` and each other's results
/// without reference counting. A worker that panics aborts the whole process (`panic = "abort"`
/// in `Cargo.toml`), so a `join` never actually returns an error. The one error the second scope
/// can meet, failing to create the output file, ends the process with a message, because the
/// other threads would otherwise wait at the barrier forever.
fn summarize(log: &[u8], out_path: &Path) -> Result<(), Box<dyn Error>> {
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let bounds = line_bounds(log, threads);
    let mut chunks = thread::scope(|s| {
        let parsers: Vec<_> = (bounds.windows(2))
            .map(|w| s.spawn(move || parse(&log[w[0]..w[1]])))
            .collect();
        (parsers.into_iter())
            .map(|p| p.join().expect("a panic aborts the process"))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let (names, ranks) = rank_endpoints(&mut chunks);
    let mut runs: Vec<Vec<Run>> = vec![Vec::new(); HOURS];
    for (chunk, rank) in chunks.iter().zip(&ranks) {
        for (hour, lines, first) in &chunk.runs {
            runs[*hour].push((&chunk.records[lines.clone()], &chunk.large[*first..], rank));
        }
    }
    // Night hours carry about half the traffic of day hours, so fixed blocks of hours would
    // leave some workers idle while others finish. Workers instead take one hour at a time from
    // a shared counter, largest first (longest-processing-time-first scheduling).
    let mut by_size: Vec<usize> = (0..HOURS).collect();
    by_size.sort_by_key(|&h| Reverse(runs[h].iter().map(|run| run.0.len()).sum::<usize>()));
    let taken = AtomicUsize::new(0);
    let (hours, sorted, places) = (Mutex::new(Vec::new()), OnceLock::new(), OnceLock::new());
    let barrier = Barrier::new(threads);
    // The same threads summarize the hours and then format them. Between the two phases they
    // wait at a barrier while one of them sorts the hours and maps the output file, which saves
    // spawning a second team: the last of twelve new threads started about 1 ms after the first.
    let shared = (
        &runs, &names, &by_size, &taken, &hours, &sorted, &places, &barrier,
    );
    thread::scope(|s| {
        for (t, chunk) in chunks.iter().enumerate() {
            s.spawn(move || {
                let (runs, names, by_size, taken, hours, sorted, places, barrier) = shared;
                let queue = std::iter::from_fn(|| by_size.get(taken.fetch_add(1, Relaxed)));
                let done = summarize_hours(queue.copied(), runs, names);
                hours
                    .lock()
                    .expect("a panic aborts the process")
                    .extend(done);
                if barrier.wait().is_leader() {
                    let hours = sorted.get_or_init(|| {
                        let mut hours = std::mem::take(&mut *hours.lock().expect("no panic"));
                        hours.sort_unstable_by_key(|hour: &Hour| hour.index);
                        hours
                    });
                    let len = hours.iter().map(|hour| hour.len).sum();
                    let mut rest = map_output(out_path, len).unwrap_or_else(|e| {
                        eprintln!("Error: {}: {e}", out_path.display());
                        std::process::exit(1)
                    });
                    let mut split = Vec::with_capacity(hours.len());
                    for hour in hours {
                        let (place, after) = rest.split_at_mut(hour.len);
                        split.push((hour, place));
                        rest = after;
                    }
                    let _ = places.set(Mutex::new(split.into_iter()));
                }
                barrier.wait();
                // Every hour has its place in the file now, so the threads format in parallel,
                // taking the next hour from a shared queue until none is left. Each first frees
                // one chunk's records: no hour needs them any more. Releasing the input waits
                // until the end, because freeing pages interrupts every other thread to flush its
                // TLB, which cost 3 ms in the hour pass.
                release(&chunk.records);
                release(&chunk.large);
                let mut text = Vec::new();
                let queue = places.get().expect("the leader mapped the output");
                let next = || queue.lock().expect("a panic aborts the process").next();
                for (hour, place) in std::iter::from_fn(next) {
                    text.clear();
                    format_hour(hour, names, &mut text);
                    place.copy_from_slice(&text); // panics if `Hour::len` was wrong
                }
                release(&log[log.len() * t / threads..log.len() * (t + 1) / threads]);
            });
        }
    });
    Ok(())
}

/// Gives the memory pages that lie entirely inside `items` back to the kernel, with
/// `madvise(MADV_DONTNEED)`.
///
/// At exit the kernel frees every page we touched, on one thread and inside the timed window:
/// for the 58,000 pages of the input alone that took 3.9 ms. So each thread frees its share of
/// the input and of the records itself, in parallel with the others, once nothing reads them.
/// A page that only partly belongs to `items` stays mapped, since its other part may still be
/// in use. The call is advice: if it fails, the exit does the work as before, so the result is
/// ignored.
fn release<T>(items: &[T]) {
    let start = items.as_ptr().addr();
    let first = start.next_multiple_of(4096);
    let end = (start + size_of_val(items)) & !4095;
    if first < end {
        let first = items.as_ptr().cast::<u8>().with_addr(first);
        // SAFETY: the pages lie inside `items`. In the input's private read-only file mapping, a
        // later read (an endpoint name, say) just maps the same file data again. Records read as
        // zeros afterwards, which is why a chunk's records are released only after every hour
        // has been summarized and nothing reads them again.
        unsafe { madvise(first, end - first.addr(), MADV_DONTNEED) };
    }
}

/// Splits `log` into `parts` chunks of whole lines and returns the `parts + 1` boundaries.
///
/// Each boundary starts at an even share of the bytes and moves forward to just past the next
/// LF, so every line lands in exactly one chunk. The boundaries never decrease, and with fewer
/// lines than parts some chunks are simply empty.
fn line_bounds(log: &[u8], parts: usize) -> Vec<usize> {
    let mut bounds = vec![0];
    for i in 1..parts {
        let at = log.len() * i / parts;
        let newline = log[at..].iter().position(|&b| b == b'\n');
        bounds.push(newline.map_or(log.len(), |p| at + p + 1));
    }
    bounds.push(log.len());
    bounds
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
/// [`rank_endpoints`] later maps them to global ranks.
struct Chunk<'a> {
    /// Local endpoint id → (name, hash). Names borrow the mapped file, so none are copied.
    names: Vec<(&'a [u8], u64)>,
    /// Local endpoint id → the padded copy of [`Interner::padded`], which lets
    /// [`rank_endpoints`] compare names without reading them from all over the input.
    padded: Vec<[u8; 64]>,
    records: Vec<u32>,
    large: Vec<u32>,
    /// The runs in file order: each one's hour, its lines' indices, and the index in `large` of
    /// its first large latency.
    runs: Vec<(usize, Range<usize>, usize)>,
}

/// The value of a record's 19-bit latency field that sends readers to the chunk's list of large
/// latencies (see [`Chunk`]).
const LARGE: u32 = (1 << 19) - 1;

/// Parses one chunk of whole lines into records.
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
fn parse(chunk: &[u8]) -> Result<Chunk<'_>, String> {
    let mut endpoints = Interner::default();
    let (mut records, mut large) = (Vec::with_capacity(chunk.len() / MIN_LINE + 1), Vec::new());
    // The hour, first line and first large latency of every run. A run lasts while the
    // timestamp's bytes 5 to 12, `MM-DDTHH`, stay the same, so most lines compare those 8 bytes
    // and decode nothing. No line's bytes are all zero, so line 0 starts a run.
    let (mut starts, mut current) = (Vec::new(), 0);
    let mut i = 0;
    while i < chunk.len() {
        let line = &chunk[i..];
        let lf = line_end(line);
        i += lf + 1;
        if word(line, 5) != current {
            let hour = hour_index(line).ok_or_else(|| {
                let ts = String::from_utf8_lossy(&line[..27]);
                format!("timestamp {ts} is outside the spec's 72-hour span")
            })?;
            starts.push((hour, records.len(), large.len()));
            current = word(line, 5);
        }
        // The method is 3 to 7 bytes at offset 28, so the space after it is in the word at 31.
        let start = 32 + spaces(word(line, 31)).trailing_zeros() as usize / 8;
        // `space` precedes the latency; before it sit the 3-digit status and its own space.
        let (latency_us, space) = latency(line, lf);
        let name = &line[start..space - 4];
        let window = line.get(start..).and_then(|w| w.first_chunk());
        let id = endpoints.intern(name, hash(name), window);
        // Errors are exactly 500..=599, so the status's first digit decides.
        let error = u32::from(line[space - 3] == b'5');
        let small = latency_us.min(LARGE);
        if small == LARGE {
            large.push(latency_us);
        }
        records.push(id << 20 | error << 19 | small);
    }
    // The spec gives every seed 1,842 endpoints (§2), well below the 4,096 that 12 bits hold.
    // Past that, the ids above were cut short, so we stop instead.
    if endpoints.names.len() > 1 << 12 {
        return Err("over 4,096 distinct endpoints in one chunk: ids overflow 12 bits".into());
    }
    let ends = starts
        .iter()
        .skip(1)
        .map(|run| run.1)
        .chain([records.len()]);
    let runs =
        (starts.iter().zip(ends)).map(|(&(hour, start, first), end)| (hour, start..end, first));
    Ok(Chunk {
        names: endpoints.names,
        padded: endpoints.padded,
        records,
        large,
        runs: runs.collect(),
    })
}

/// Returns the index of the LF that ends the first line of `line`.
///
/// No line is shorter than [`MIN_LINE`], so the search starts at byte 39 rather than 0. From
/// there one 64-byte window, checked with two AVX2 compares in [`equal_mask`], holds the LF of
/// about 90% of lines, which makes the branch after it easy to predict. Longer lines take
/// another window, and the last line of a chunk, with fewer than 64 bytes left, falls back to a
/// plain byte search.
fn line_end(line: &[u8]) -> usize {
    let mut at = MIN_LINE - 1;
    while at + 64 <= line.len() {
        let window = line[at..at + 64].try_into().expect("64-byte window");
        // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
        let newlines = unsafe { equal_mask(window, &[b'\n'; 64]) };
        if newlines != 0 {
            return at + newlines.trailing_zeros() as usize;
        }
        at += 64;
    }
    let tail = line[at..].iter().position(|&b| b == b'\n');
    at + tail.expect("every line ends in LF (spec §2)")
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
    use std::arch::x86_64::{_mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8};
    let mut mask = 0;
    for offset in [0, 32] {
        // SAFETY: both 32-byte unaligned loads lie inside their 64-byte blocks.
        let (a, b) = unsafe {
            (
                _mm256_loadu_si256(a.as_ptr().add(offset).cast()),
                _mm256_loadu_si256(b.as_ptr().add(offset).cast()),
            )
        };
        let equal = _mm256_movemask_epi8(_mm256_cmpeq_epi8(a, b)).cast_unsigned();
        mask |= u64::from(equal) << offset;
    }
    mask
}

/// Returns the hour index `0..72` of a line's timestamp, or `None` outside the span.
///
/// The timestamp is fixed-width, so we read the month, day and hour digits straight from their
/// offsets instead of parsing a date. The span starts at 18:00 on October 30, so the days 10-30,
/// 10-31, 11-01 and 11-02 become 0 to 3, and the index is `day × 24 + hour − 18`. For example
/// `2026-11-01T00:...` is index 30.
fn hour_index(line: &[u8]) -> Option<usize> {
    let two = |i: usize| usize::from(line[i] - b'0') * 10 + usize::from(line[i + 1] - b'0');
    let day = match &line[5..7] {
        b"10" => two(8).checked_sub(30)?,
        b"11" => two(8) + 1,
        _ => return None,
    };
    let hour = (day * 24 + two(11)).checked_sub(18)?;
    (hour < HOURS).then_some(hour)
}

/// Returns the latency that ends at `lf`, and the index of the space before it.
///
/// Latencies of up to 7 digits (99.9% of lines) take one step without a loop. We load the 8
/// bytes before the LF as one integer, find the last space in it with [`spaces`], zero every
/// byte up to and including that space, and convert the remaining digits with
/// [`eight_digits`]. For a line ending in `200 1234`, the word holds exactly those 8 bytes, the
/// last space is byte 3, and the digit lanes left are `0 0 0 0 1 2 3 4`.
fn latency(line: &[u8], lf: usize) -> (u32, usize) {
    let w = word(line, lf - 8);
    let found = spaces(w);
    if found == 0 {
        // 8 to 10 digits: rare enough for a plain loop.
        let space = line[..lf].iter().rposition(|&b| b == b' ');
        let space = space.expect("a space precedes the latency");
        let digits = line[space + 1..lf].iter();
        return (digits.fold(0, |v, &d| v * 10 + u32::from(d - b'0')), space);
    }
    let last_space = 7 - found.leading_zeros() as usize / 8;
    let digit_lanes = u64::MAX << (8 * (last_space + 1));
    let digits = (w & digit_lanes) - (0x3030_3030_3030_3030 & digit_lanes);
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

/// Returns a mask with bit 7 of every byte set where that byte of `w` is a space.
///
/// XOR with `0x20` turns spaces into zero bytes, so the task becomes "which bytes are zero".
/// For one byte `x`, `(x & 0x7F) + 0x7F` sets bit 7 exactly when the low seven bits are not all
/// zero, and OR-ing `x` itself also catches bytes whose bit 7 was already set. So bit 7 ends up
/// clear only for zero bytes, and the final NOT and mask report those. The addition never
/// carries out of a byte, so every byte's answer is exact. The shorter, better-known zero-byte
/// test from "Bit Twiddling Hacks" can flag false matches above a real one, and [`latency`]
/// needs the *last* space, so it uses this exact form.
fn spaces(w: u64) -> u64 {
    let x = w ^ 0x2020_2020_2020_2020;
    !(((x & 0x7F7F_7F7F_7F7F_7F7F) + 0x7F7F_7F7F_7F7F_7F7F) | x) & 0x8080_8080_8080_8080
}

/// Hashes an endpoint name for the [`Interner`].
///
/// Hashing every byte would need a loop whose length varies from line to line. Instead we mix
/// the length with three 8-byte windows: the first, the middle and the last. That is a fixed
/// amount of work, and it covers every byte of names up to 24 bytes long. Each window lies
/// inside the name, so equal names always hash equal. Two different names that agree on all
/// three windows would only share a probe sequence: the interner still compares the bytes.
/// The mixing step is the multiply-rotate used by rustc's `FxHasher`.
fn hash(name: &[u8]) -> u64 {
    let mix = |hash: u64, w: u64| (hash.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    let n = name.len();
    if n < 8 {
        let mut short = [0; 8];
        short[..n].copy_from_slice(name);
        return mix(n as u64, u64::from_le_bytes(short));
    }
    let first_and_middle = mix(mix(n as u64, word(name, 0)), word(name, n / 2 - 4));
    mix(first_and_middle, word(name, n - 8))
}

/// A hash table that gives every distinct endpoint name a dense id: 0, 1, 2, and so on.
///
/// It uses open addressing with linear probing. A name's hash picks a slot, and on a collision
/// we try the next slot until we find the name or an empty slot. The table doubles whenever it
/// gets a quarter full, which assumes nothing about how many distinct endpoints a seed has. At
/// half full, 7.8% of seed 1's lines needed a second probe, each a mispredicted branch; at a
/// quarter, 3.4% do.
struct Interner<'a> {
    /// The table. A slot holds `id + 1`, so that 0 can mark an empty slot.
    slots: Vec<u32>,
    /// Id → (name, hash). The hash lets the table grow, and [`rank_endpoints`] merge the chunks'
    /// names, without hashing anything again.
    names: Vec<(&'a [u8], u64)>,
    /// Id → the name and the space after it, zero-padded to 64 bytes, for [`same_name`]. A name of
    /// 64 bytes or more has no room for the space and keeps only its first 64 bytes.
    padded: Vec<[u8; 64]>,
}

impl Default for Interner<'_> {
    fn default() -> Self {
        Self {
            slots: vec![0; 1024],
            names: Vec::new(),
            padded: Vec::new(),
        }
    }
}

// `intern` and `slot` run once per line. As calls, with their register saves and bounds checks,
// they cost 9% of all instructions, so they are forced inline; `insert` stays out of line.
#[expect(clippy::inline_always, reason = "measured: about 1 ms of wall time")]
impl<'a> Interner<'a> {
    /// Returns `name`'s id, assigning the next free one if the name is new.
    ///
    /// `window` is the 64 bytes that start with `name` and the space after it, when the input has
    /// that many left. Every line but the last one of a chunk has them, and with them a probe
    /// compares a name of up to 63 bytes without a call to `memcmp` (see [`same_name`]). That
    /// covers 1,791 of the 1,842 endpoints of seed 1 and 94% of its lines; longer names compare
    /// with `==`.
    #[inline(always)]
    fn intern(&mut self, name: &'a [u8], hash: u64, window: Option<&[u8; 64]>) -> u32 {
        let slot = self.slot(name, hash, window);
        match self.slots[slot].checked_sub(1) {
            Some(id) => id,
            None => self.insert(slot, name, hash),
        }
    }

    /// Gives `name`, which belongs in the empty `slot`, the next id. New names are rare (one
    /// per distinct endpoint per chunk), so this stays out of line, away from the hot loop.
    #[cold]
    #[inline(never)]
    fn insert(&mut self, slot: usize, name: &'a [u8], hash: u64) -> u32 {
        // The name, then the space if there is room for it, then zeros.
        let mut padded = [b' '; 64];
        let kept = name.len().min(64);
        padded[..kept].copy_from_slice(&name[..kept]);
        padded[kept.min(63) + 1..].fill(0);
        self.padded.push(padded);
        self.names.push((name, hash));
        self.slots[slot] = self.names.len() as u32;
        if self.names.len() * 4 > self.slots.len() {
            let grown = vec![0; self.slots.len() * 2];
            let old = std::mem::replace(&mut self.slots, grown);
            for id in old.into_iter().filter(|&id| id != 0) {
                let (name, hash) = self.names[id as usize - 1];
                let slot = self.slot(name, hash, None);
                self.slots[slot] = id;
            }
        }
        self.names.len() as u32 - 1
    }

    /// Returns the slot that holds `name`, or the empty slot where it belongs.
    #[inline(always)]
    fn slot(&self, name: &[u8], hash: u64, window: Option<&[u8; 64]>) -> usize {
        let mask = self.slots.len() - 1;
        let mut slot = (hash >> 32) as usize & mask;
        while let Some(id) = self.slots[slot].checked_sub(1) {
            let found = match window {
                Some(window) if name.len() < 64 => {
                    same_name(window, &self.padded[id as usize], name.len())
                }
                _ => self.names[id as usize].0 == name,
            };
            if found {
                break;
            }
            slot = (slot + 1) & mask;
        }
        slot
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
/// `len`, and one shorter has its space where the window still holds part of the name. The copies are 64 bytes, not the
/// 125 a name can reach, because half the size keeps more of them in cache: that was worth 0.8 ms.
fn same_name(window: &[u8; 64], known: &[u8; 64], len: usize) -> bool {
    // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
    !unsafe { equal_mask(window, known) } & u64::MAX >> (63 - len) == 0
}

/// Assigns every endpoint its global rank: its position in the bytewise sort of all names.
///
/// Each chunk has its own ids, so we first merge the chunks' names through one more
/// [`Interner`], which drops duplicates in a single pass instead of sorting about 22,000
/// names. Only the 1,842 distinct names are then sorted. Rust's slice ordering is bytewise and
/// puts a proper prefix first, which is exactly the order the spec asks for (§4). Returns the
/// sorted names and, for each chunk, a table from its local ids to ranks.
///
/// The merge compares the chunks' padded copies, which then go: each name's bytes in the input
/// lie in a different place, and reading them there took most of a millisecond, one cache miss
/// after another, while every other thread waited.
fn rank_endpoints<'a>(chunks: &mut [Chunk<'a>]) -> (Vec<&'a [u8]>, Vec<Vec<u32>>) {
    let mut all = Interner::default();
    let global: Vec<Vec<u32>> = (chunks.iter_mut())
        .map(|c| {
            let padded = std::mem::take(&mut c.padded);
            (c.names.iter().zip(&padded))
                .map(|(&(name, hash), padded)| all.intern(name, hash, Some(padded)))
                .collect()
        })
        .collect();
    let mut order: Vec<u32> = (0..all.names.len() as u32).collect();
    order.sort_unstable_by_key(|&id| all.names[id as usize].0);
    let mut rank = vec![0; order.len()];
    for (r, &id) in order.iter().enumerate() {
        rank[id as usize] = r as u32;
    }
    let names = order.iter().map(|&id| all.names[id as usize].0).collect();
    let ranks = (global.into_iter())
        .map(|ids| ids.into_iter().map(|id| rank[id as usize]).collect())
        .collect();
    (names, ranks)
}

/// One hour of output, ready to format: [`summarize_hours`] fills it, [`format_hour`] writes it.
struct Hour {
    /// The hour index, `0..72`.
    index: usize,
    /// One entry per output line, in output order: the endpoint's rank, then the request count,
    /// the p50, p95 and p99 latencies and the error count.
    lines: Vec<[u32; 6]>,
    /// The exact length of the formatted hour, in bytes, so that its place in the output file
    /// is known before any hour is formatted.
    len: usize,
}

/// One run of lines from one hour: their records, their chunk's large latencies from the run's
/// first one on (see [`Chunk`]), and the table from their chunk's endpoint ids to ranks.
type Run<'a> = (&'a [u32], &'a [u32], &'a [u32]);

/// Computes the output lines of each hour in `hours`, from `runs[hour]`.
///
/// For each hour we go through that hour's runs from every chunk, and then:
///
/// 1. **Count** the lines and errors of each endpoint (by rank).
/// 2. **Place** each latency in one buffer at its endpoint's next free position. The prefix sum
///    of the counts tells each endpoint where its run starts. Together, steps 1 and 2 are a
///    counting sort by endpoint.
/// 3. **Select** the three percentiles from each endpoint's run.
///
/// Endpoints are visited in rank order, so the lines come out already in output order.
fn summarize_hours(
    hours: impl Iterator<Item = usize>,
    runs: &[Vec<Run>],
    names: &[&[u8]],
) -> Vec<Hour> {
    let mut done = Vec::new();
    // After the counting pass, endpoint e's latencies go to latencies[start[e]..start[e + 1]].
    let mut start = vec![0u32; names.len() + 1];
    let mut next = vec![0u32; names.len()];
    let mut errors = vec![0u32; names.len()];
    let mut latencies = Vec::new();
    for hour in hours {
        let mut lines = Vec::new();
        let mut len = 0;
        start.fill(0);
        errors.fill(0);
        for &(records, _, rank) in &runs[hour] {
            for &r in records {
                let e = rank[(r >> 20) as usize] as usize;
                start[e + 1] += 1;
                errors[e] += r >> 19 & 1;
            }
        }
        for e in 0..names.len() {
            start[e + 1] += start[e];
        }
        next.copy_from_slice(&start[..names.len()]);
        latencies.resize(start[names.len()] as usize, 0);
        for &(records, large, rank) in &runs[hour] {
            let mut large = large.iter().copied();
            for &r in records {
                let e = rank[(r >> 20) as usize] as usize;
                latencies[next[e] as usize] = match r & LARGE {
                    LARGE => large.next().expect("one large latency per LARGE record"),
                    small => small,
                };
                next[e] += 1;
            }
        }
        let label = hour_label(hour).len();
        for (e, name) in names.iter().enumerate() {
            let group = &mut latencies[start[e] as usize..start[e + 1] as usize];
            if group.is_empty() {
                continue;
            }
            let n = group.len();
            // The spec's nearest-rank percentile (§4) is the value at 1-indexed rank
            // ceil(p·n / 100), which it writes as (p·n + 99) / 100. We subtract 1 for a 0-based
            // index.
            let index = |p: usize| (p * n).div_ceil(100) - 1;
            // `select_nth_unstable(k)` puts the k-th smallest value at index k, everything
            // smaller before it and everything larger after it, in linear time. Selecting the
            // median first leaves both higher ranks in the upper half, and p95 then leaves p99
            // in the top 5%, so the three selections cover about 1.55 n values instead of the
            // 2.94 n of going from p99 down. The suffixes start at the rank already found
            // because the ranks coincide for small groups (at n = 1 all three are index 0).
            let (i50, i95, i99) = (index(50), index(95), index(99));
            let p50 = *group.select_nth_unstable(i50).1;
            let p95 = *group[i50..].select_nth_unstable(i95 - i50).1;
            let p99 = *group[i95..].select_nth_unstable(i99 - i95).1;
            let line = [e as u32, n as u32, p50, p95, p99, errors[e]];
            // The label, a space, the name, six numbers that each follow a space, and the LF.
            let numbers = numbers(&line).map(|value| 1 + digits(value));
            len += label + 1 + name.len() + numbers.iter().sum::<usize>() + 1;
            lines.push(line);
        }
        done.push(Hour {
            index: hour,
            lines,
            len,
        });
    }
    done
}

/// The six numbers an output line prints after the endpoint's name (spec §4): the request
/// count, p50, p95, p99, the error count and the errors per million requests, rounded down.
fn numbers(&[_, count, p50, p95, p99, errors]: &[u32; 6]) -> [u64; 6] {
    let (count, errors) = (u64::from(count), u64::from(errors));
    [
        count,
        p50.into(),
        p95.into(),
        p99.into(),
        errors,
        errors * 1_000_000 / count,
    ]
}

/// The number of decimal digits [`push_number`] writes for `value`.
fn digits(value: u64) -> usize {
    value.checked_ilog10().map_or(1, |log| log as usize + 1)
}

/// Appends the formatted lines of `hour` to `out`: `<hour> <endpoint>` and the six numbers.
fn format_hour(hour: &Hour, names: &[&[u8]], out: &mut Vec<u8>) {
    let label = hour_label(hour.index);
    for line in &hour.lines {
        out.extend_from_slice(label.as_bytes());
        out.push(b' ');
        out.extend_from_slice(names[line[0] as usize]);
        for value in numbers(line) {
            push_number(out, value);
        }
        out.push(b'\n');
    }
}

/// Appends a space and then `value` in decimal.
///
/// About 760,000 numbers go through here. `write!` runs each one through Rust's general
/// formatting machinery, which costs more than producing the digits, so we write them directly:
/// least significant first, into a small buffer from the back.
fn push_number(out: &mut Vec<u8>, mut value: u64) {
    let mut digits = [0; 20];
    let mut at = digits.len();
    loop {
        at -= 1;
        digits[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    out.push(b' ');
    out.extend_from_slice(&digits[at..]);
}

/// Returns the output label `YYYY-MM-DDTHH:00:00Z` of hour index `hour`, the inverse of
/// [`hour_index`].
fn hour_label(hour: usize) -> String {
    let (day, hh) = ((hour + 18) / 24, (hour + 18) % 24);
    let (month, dd) = if day < 2 {
        (10, 30 + day)
    } else {
        (11, day - 1)
    };
    format!("2026-{month:02}-{dd:02}T{hh:02}:00:00Z")
}
