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
//! 2. **Parse once.** Each thread turns every line of its chunk into one 8-byte record: endpoint
//!    id, hour, error bit and latency ([`parse`]). Endpoint names become small integer ids
//!    through a per-thread hash table ([`Interner`]). The thread then sorts its records by hour,
//!    in place ([`sort_by_hour`]).
//! 3. **Merge.** We collect every thread's endpoint names, drop duplicates and sort them bytewise
//!    ([`rank_endpoints`]). An endpoint's position in that list, its *rank*, is also its position
//!    within an hour of the output, so the output never needs sorting.
//! 4. **Summarize by hour.** Threads take hours one at a time, largest first
//!    ([`summarize_hours`]). For one hour a thread counts the lines per endpoint, uses the counts
//!    to place each latency next to the others of its endpoint (a counting sort), and picks the
//!    three percentiles out of every run with a selection algorithm, which is linear, instead of
//!    a full sort. What it keeps is six numbers per output line, and the hour's exact length
//!    once formatted.
//! 5. **Write in place.** With every hour's length known, we give the output file its final size
//!    and map it into memory ([`map_output`]). Each thread then frees one chunk's records and
//!    formats its run of hours straight into place in the file ([`format_hour`]).
//!
//! ## Why it is fast
//!
//! - **One pass over the input.** The log is 239 MB, ten times the L3 cache, so every pass over
//!   it streams from main memory. We parse it once and keep 25 MB of compact records instead.
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

use std::cmp::Reverse;
use std::error::Error;
use std::fs::File;
use std::num::NonZeroUsize;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::thread;

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
    // Mapping a file for writing needs it open for reading too.
    let file = File::options().read(true).write(true).create(true).truncate(true).open(path)?;
    if len == 0 {
        return Ok(&mut []); // mmap rejects a zero length
    }
    file.set_len(u64::try_from(len)?)?;
    // SAFETY: a new shared read-write mapping of a file we just opened read-write and sized to
    // `len`; the kernel picks the address.
    let ptr = unsafe {
        let prot = PROT_READ | PROT_WRITE;
        mmap(std::ptr::null_mut(), len, prot, MAP_SHARED, file.as_raw_fd(), 0)
    };
    if ptr.addr() == usize::MAX {
        return Err(format!("mmap: {}", std::io::Error::last_os_error()).into());
    }
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
    // SAFETY: a new read-only private mapping of an open fd; the kernel picks the address.
    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            len,
            PROT_READ,
            MAP_PRIVATE,
            file.as_raw_fd(),
            0,
        )
    };
    if ptr.addr() == usize::MAX {
        let err = std::io::Error::last_os_error();
        return Err(format!("mmap {}: {err}", path.display()).into());
    }
    // SAFETY: the mapping holds `len` readable bytes and is never unmapped, so it outlives every
    // borrow. /data is mounted read-only, so the bytes cannot change under us.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Runs the whole pipeline over `log` and writes the output to `out_path`.
///
/// All three parallel phases use [`std::thread::scope`], so the threads can borrow `log` and
/// each other's results without reference counting. A worker that panics aborts the whole
/// process (`panic = "abort"` in `Cargo.toml`), so a `join` never actually returns an error.
fn summarize(log: &[u8], out_path: &Path) -> Result<(), Box<dyn Error>> {
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let bounds = line_bounds(log, threads);
    let chunks = thread::scope(|s| {
        let parsers: Vec<_> = (bounds.windows(2))
            .map(|w| s.spawn(move || parse(&log[w[0]..w[1]])))
            .collect();
        (parsers.into_iter())
            .map(|p| p.join().expect("a panic aborts the process"))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let (names, ranks) = rank_endpoints(&chunks);
    // Night hours carry about half the traffic of day hours, so fixed blocks of hours would
    // leave some workers idle while others finish. Workers instead take one hour at a time from
    // a shared counter, largest first (longest-processing-time-first scheduling).
    let mut by_size: Vec<usize> = (0..HOURS).collect();
    by_size.sort_by_key(|&h| Reverse(chunks.iter().map(|c| c.hour(h).len()).sum::<usize>()));
    let taken = AtomicUsize::new(0);
    let (chunks, ranks, names, by_size, taken) = (&chunks, &ranks, &names, &by_size, &taken);
    let mut hours: Vec<Hour> = thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(move || {
                    let queue = std::iter::from_fn(|| by_size.get(taken.fetch_add(1, Relaxed)));
                    summarize_hours(queue.copied(), chunks, ranks, names)
                })
            })
            .collect();
        (workers.into_iter())
            .flat_map(|w| w.join().expect("a panic aborts the process"))
            .collect()
    });
    hours.sort_unstable_by_key(|hour| hour.index);
    let len = hours.iter().map(|hour| hour.len).sum();
    let mut rest = map_output(out_path, len).map_err(|e| format!("{}: {e}", out_path.display()))?;
    let mut places = Vec::with_capacity(hours.len());
    for hour in &hours {
        let (place, after) = rest.split_at_mut(hour.len);
        places.push((hour, place));
        rest = after;
    }
    // Every hour has its place in the file now, so the threads format in parallel, taking the
    // next hour from a shared queue until none is left. Each first frees one chunk's records:
    // no hour needs them any more. Releasing the input waits until the end, because freeing
    // pages interrupts every other thread to flush its TLB, which cost 3 ms in the hour pass.
    let queue = Mutex::new(places.into_iter());
    let queue = &queue;
    thread::scope(|s| {
        for (t, chunk) in chunks.iter().enumerate() {
            s.spawn(move || {
                release(&chunk.records);
                let mut text = Vec::new();
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
    let (first, end) = (start.next_multiple_of(4096), (start + size_of_val(items)) & !4095);
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
/// Every line becomes a single `u64` record, packed like this:
///
/// | bits    | field                                       |
/// |---------|---------------------------------------------|
/// | 63 – 40 | the endpoint's id in this chunk (24 bits)   |
/// | 39 – 33 | the hour index, `0..72` (7 bits)            |
/// | 32      | 1 if the status is 500–599                  |
/// | 31 – 0  | the latency in microseconds                 |
///
/// Eight bytes per line keep all 3.1 million records at 25 MB. The ids are local to the chunk:
/// [`rank_endpoints`] later maps them to global ranks.
struct Chunk<'a> {
    /// Local endpoint id → (name, hash). Names borrow the mapped file, so none are copied.
    names: Vec<(&'a [u8], u64)>,
    /// One record per line, sorted by hour.
    records: Vec<u64>,
    /// `records[hour_start[h]..hour_start[h + 1]]` are hour `h`'s lines.
    hour_start: [usize; HOURS + 1],
}

impl Chunk<'_> {
    /// The records of one hour.
    fn hour(&self, hour: usize) -> &[u64] {
        &self.records[self.hour_start[hour]..self.hour_start[hour + 1]]
    }
}

/// Parses one chunk of whole lines into records, then sorts them by hour.
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
    let mut records = Vec::with_capacity(chunk.len() / MIN_LINE + 1);
    let mut per_hour = [0; HOURS];
    let mut i = 0;
    while i < chunk.len() {
        let line = &chunk[i..];
        let lf = line_end(line);
        i += lf + 1;
        let hour = hour_index(line).ok_or_else(|| {
            let ts = String::from_utf8_lossy(&line[..27]);
            format!("timestamp {ts} is outside the spec's 72-hour span")
        })?;
        // The method is 3 to 7 bytes at offset 28, so the space after it is in the word at 31.
        let start = 32 + spaces(word(line, 31)).trailing_zeros() as usize / 8;
        // `space` precedes the latency; before it sit the 3-digit status and its own space.
        let (latency_us, space) = latency(line, lf);
        let name = &line[start..space - 4];
        let id = endpoints.intern(name, hash(name), line.get(start..).and_then(|w| w.first_chunk()));
        // Errors are exactly 500..=599, so the status's first digit decides.
        let error = u64::from(line[space - 3] == b'5');
        per_hour[hour] += 1;
        records
            .push(u64::from(id) << 40 | (hour as u64) << 33 | error << 32 | u64::from(latency_us));
    }
    if endpoints.names.len() > 1 << 24 {
        return Err("over 2^24 distinct endpoints in one chunk: ids overflow the record".into());
    }
    let hour_start = sort_by_hour(&mut records, &per_hour);
    Ok(Chunk {
        names: endpoints.names,
        records,
        hour_start,
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
/// gets half full, which keeps probe sequences short and means it assumes nothing about how many
/// distinct endpoints a seed has.
struct Interner<'a> {
    /// The table. A slot holds `id + 1`, so that 0 can mark an empty slot.
    slots: Vec<u32>,
    /// Id → (name, hash). Keeping the hash lets a probe reject most non-matching slots without
    /// comparing bytes, and lets the table grow without hashing anything again.
    names: Vec<(&'a [u8], u64)>,
    /// Id → the name's bytes, zero-padded to 128, for [`same_name`].
    padded: Vec<[u8; 128]>,
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
    /// `window` is the 128 bytes that start with `name` when the input has that many left. Every
    /// line but the last one or two of a chunk has them, and with them a probe compares names
    /// without a call to `memcmp` (see [`same_name`]).
    #[inline(always)]
    fn intern(&mut self, name: &'a [u8], hash: u64, window: Option<&[u8; 128]>) -> u32 {
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
        let mut padded = [0; 128];
        let kept = name.len().min(128);
        padded[..kept].copy_from_slice(&name[..kept]);
        self.padded.push(padded);
        self.names.push((name, hash));
        self.slots[slot] = self.names.len() as u32;
        if self.names.len() * 2 > self.slots.len() {
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
    fn slot(&self, name: &[u8], hash: u64, window: Option<&[u8; 128]>) -> usize {
        let mask = self.slots.len() - 1;
        let mut slot = (hash >> 32) as usize & mask;
        while let Some(id) = self.slots[slot].checked_sub(1) {
            let (known, known_hash) = self.names[id as usize];
            if known_hash == hash
                && match window {
                    Some(window) if name.len() <= 128 => {
                        same_name(window, &self.padded[id as usize], name.len())
                    }
                    _ => known == name,
                }
            {
                break;
            }
            slot = (slot + 1) & mask;
        }
        slot
    }
}

/// Returns whether the first `len` bytes of `window` and `known` agree, for `len <= 128`.
///
/// Names vary in length from line to line, and the branches inside `memcmp` that pick a path
/// by length mispredict often; that was about a quarter of the parse loop. Here the length
/// never steers a branch for names of up to 64 bytes: we compare all 64 bytes of both blocks at
/// once and ignore the result for bytes past `len`. Bytes past the name differ, of course (the
/// window goes on into the status and the latency, the padded copy holds zeros), which is
/// why they are masked out: `u64::MAX >> (64 - len)` keeps bit `i` exactly for `i < len`.
fn same_name(window: &[u8; 128], known: &[u8; 128], len: usize) -> bool {
    let (window, known) = (window.as_chunks::<64>().0, known.as_chunks::<64>().0);
    let wanted = |len: usize| u64::MAX.checked_shr(64 - len.min(64) as u32).unwrap_or(0);
    // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
    let low = unsafe { equal_mask(&window[0], &known[0]) };
    if len <= 64 {
        return !low & wanted(len) == 0;
    }
    // SAFETY: as above.
    let high = unsafe { equal_mask(&window[1], &known[1]) };
    !low == 0 && !high & wanted(len - 64) == 0
}

/// Sorts `records` by hour in place and returns where each hour starts.
///
/// This is an American flag sort: a counting sort that permutes in place instead of copying into
/// a second buffer, so it needs no extra memory. The counts from parsing give each hour its
/// region of the array. Then we walk the regions in order, and every record that sits in the
/// wrong region is swapped straight into the next free place of its own. Each swap puts at least
/// one record where it belongs, so the whole sort is linear. It takes about 1.6 ms per thread.
fn sort_by_hour(records: &mut [u64], per_hour: &[usize; HOURS]) -> [usize; HOURS + 1] {
    let mut start = [0; HOURS + 1];
    for h in 0..HOURS {
        start[h + 1] = start[h] + per_hour[h];
    }
    let mut next = start;
    for h in 0..HOURS {
        while next[h] < start[h + 1] {
            let to = (records[next[h]] >> 33) as usize & 127;
            if to != h {
                records.swap(next[h], next[to]);
            }
            next[to] += 1;
        }
    }
    start
}

/// Assigns every endpoint its global rank: its position in the bytewise sort of all names.
///
/// Each chunk has its own ids, so we first merge the chunks' names through one more
/// [`Interner`], which drops duplicates in a single pass instead of sorting about 22,000
/// names. Only the 1,842 distinct names are then sorted. Rust's slice ordering is bytewise and
/// puts a proper prefix first, which is exactly the order the spec asks for (§4). Returns the
/// sorted names and, for each chunk, a table from its local ids to ranks.
fn rank_endpoints<'a>(chunks: &[Chunk<'a>]) -> (Vec<&'a [u8]>, Vec<Vec<u32>>) {
    let mut all = Interner::default();
    let global: Vec<Vec<u32>> = (chunks.iter())
        .map(|c| {
            c.names
                .iter()
                .map(|&(name, hash)| all.intern(name, hash, None))
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

/// Computes the output lines of each hour in `hours`.
///
/// For each hour we gather that hour's records from every chunk, in chunk order, and then:
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
    chunks: &[Chunk],
    ranks: &[Vec<u32>],
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
        for (chunk, rank) in chunks.iter().zip(ranks) {
            for &r in chunk.hour(hour) {
                let e = rank[(r >> 40) as usize] as usize;
                start[e + 1] += 1;
                errors[e] += (r >> 32) as u32 & 1;
            }
        }
        for e in 0..names.len() {
            start[e + 1] += start[e];
        }
        next.copy_from_slice(&start[..names.len()]);
        latencies.resize(start[names.len()] as usize, 0);
        for (chunk, rank) in chunks.iter().zip(ranks) {
            for &r in chunk.hour(hour) {
                let e = rank[(r >> 40) as usize] as usize;
                latencies[next[e] as usize] = r as u32;
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
            // highest rank first leaves every lower rank inside the prefix before it, so each
            // later selection works on a shorter slice. The prefixes are inclusive because the
            // ranks coincide for small groups (at n = 1 all three are index 0).
            let p99 = *group.select_nth_unstable(index(99)).1;
            let p95 = *group[..=index(99)].select_nth_unstable(index(95)).1;
            let p50 = *group[..=index(95)].select_nth_unstable(index(50)).1;
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
    [count, p50.into(), p95.into(), p99.into(), errors, errors * 1_000_000 / count]
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
