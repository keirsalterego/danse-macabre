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
//! The work runs in two parallel phases with a short serial merge between them. Each parallel
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
//!    a full sort.
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
//! - **No teardown.** We exit right after the write, without running destructors.
//!
//! Every step's measurements are in `PERF.md`.

use std::cmp::Reverse;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::num::NonZeroUsize;
use std::os::fd::AsRawFd;
use std::path::Path;
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

// We declare `mmap` ourselves instead of pulling in the `libc` crate: every dependency costs
// points on the D axis, and the C library is already linked into every Rust binary. The two
// constants are the values from Linux's `<sys/mman.h>`.
unsafe extern "C" {
    fn madvise(addr: *const u8, len: usize, advice: i32) -> i32;
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
}
const PROT_READ: i32 = 1;
const MAP_PRIVATE: i32 = 2;
const MADV_DONTNEED: i32 = 4;

/// Reads `<data_dir>/requests.log` and writes `<out_dir>/summary.txt`.
///
/// The directories default to `/data` and `/out`, where the scoring harness mounts them; the
/// local test scripts pass their own.
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let data_dir = args.next().unwrap_or_else(|| "/data".into());
    let out_dir = args.next().unwrap_or_else(|| "/out".into());
    let log = map_file(&Path::new(&data_dir).join("requests.log"))?;
    let parts = summarize(log)?;
    let out_path = Path::new(&out_dir).join("summary.txt");
    let mut out = File::create(&out_path).map_err(|e| format!("{}: {e}", out_path.display()))?;
    for part in &parts {
        out.write_all(part)?;
    }
    // Exit without running destructors: freeing tens of megabytes of records and output buffers
    // would only add to the timed wall clock, and the kernel reclaims the memory anyway. `File`
    // keeps no user-space buffer, so every byte has already reached the kernel.
    std::process::exit(0)
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

/// Runs the whole pipeline over `log` and returns the output in pieces, one per thread, to be
/// written in order.
///
/// Both parallel phases use [`std::thread::scope`], so the threads can borrow `log` and each
/// other's results without reference counting. A worker that panics aborts the whole process
/// (`panic = "abort"` in `Cargo.toml`), so a `join` never actually returns an error.
fn summarize(log: &[u8]) -> Result<Vec<Vec<u8>>, String> {
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
    let mut hours: Vec<(usize, Vec<u8>)> = thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let queue = std::iter::from_fn(|| by_size.get(taken.fetch_add(1, Relaxed)));
                    let out = summarize_hours(queue.copied(), chunks, ranks, names);
                    release(&log[log.len() * t / threads..log.len() * (t + 1) / threads]);
                    out
                })
            })
            .collect();
        (workers.into_iter())
            .flat_map(|w| w.join().expect("a panic aborts the process"))
            .collect()
    });
    hours.sort_unstable_by_key(|&(hour, _)| hour);
    Ok(hours.into_iter().map(|(_, out)| out).collect())
}

/// Drops our page-table entries for `bytes` with `madvise(MADV_DONTNEED)`.
///
/// At exit the kernel unmaps every page we touched, on one thread and inside the timed window:
/// for the 58,000 pages of the input that took 3.9 ms. Each worker instead releases its share of
/// the input as soon as it is done, in parallel with the others. The file stays in the page
/// cache, so a later read of these bytes (an endpoint name, say) just maps its page again. The
/// call is advice: if it fails, the exit does the work as before, so the result is ignored.
fn release(bytes: &[u8]) {
    let start = bytes.as_ptr().map_addr(|addr| addr & !4095);
    // SAFETY: the range lies inside our private, read-only file mapping (rounded down to its
    // page, which is also mapped). Dropping such pages never changes their contents: the next
    // access maps the same file data again, so every `&[u8]` into it stays valid.
    unsafe {
        madvise(
            start,
            bytes.len() + (bytes.as_ptr().addr() - start.addr()),
            MADV_DONTNEED,
        )
    };
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
        let id = endpoints.intern(name, hash(name));
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
/// there one 64-byte window, checked with two AVX2 compares in [`newline_mask`], holds the LF of
/// about 90% of lines, which makes the branch after it easy to predict. Longer lines take
/// another window, and the last line of a chunk, with fewer than 64 bytes left, falls back to a
/// plain byte search.
fn line_end(line: &[u8]) -> usize {
    let mut at = MIN_LINE - 1;
    while at + 64 <= line.len() {
        let window = line[at..at + 64].try_into().expect("64-byte window");
        // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
        let newlines = unsafe { newline_mask(window) };
        if newlines != 0 {
            return at + newlines.trailing_zeros() as usize;
        }
        at += 64;
    }
    let tail = line[at..].iter().position(|&b| b == b'\n');
    at + tail.expect("every line ends in LF (spec §2)")
}

/// Returns a mask with bit `i` set wherever `block[i]` is an LF.
///
/// AVX2 compares 32 bytes at once: `_mm256_cmpeq_epi8` turns every matching byte into `0xFF`,
/// and `_mm256_movemask_epi8` gathers the top bit of each byte into a 32-bit integer. Two of
/// those cover the 64-byte block, and `trailing_zeros` on the result gives the first match.
#[target_feature(enable = "avx2")]
fn newline_mask(block: &[u8; 64]) -> u64 {
    use std::arch::x86_64::{
        _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
    };
    let lf = _mm256_set1_epi8(b'\n'.cast_signed());
    // SAFETY: both 32-byte unaligned loads lie inside the 64-byte `block`.
    let (low, high) = unsafe {
        let at = block.as_ptr();
        (
            _mm256_loadu_si256(at.cast()),
            _mm256_loadu_si256(at.add(32).cast()),
        )
    };
    let low = _mm256_movemask_epi8(_mm256_cmpeq_epi8(low, lf)).cast_unsigned();
    let high = _mm256_movemask_epi8(_mm256_cmpeq_epi8(high, lf)).cast_unsigned();
    u64::from(low) | (u64::from(high) << 32)
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
}

impl Default for Interner<'_> {
    fn default() -> Self {
        Self {
            slots: vec![0; 1024],
            names: Vec::new(),
        }
    }
}

impl<'a> Interner<'a> {
    /// Returns `name`'s id, assigning the next free one if the name is new.
    fn intern(&mut self, name: &'a [u8], hash: u64) -> u32 {
        let slot = self.slot(name, hash);
        if self.slots[slot] != 0 {
            return self.slots[slot] - 1;
        }
        self.names.push((name, hash));
        self.slots[slot] = self.names.len() as u32;
        if self.names.len() * 2 > self.slots.len() {
            let grown = vec![0; self.slots.len() * 2];
            let old = std::mem::replace(&mut self.slots, grown);
            for id in old.into_iter().filter(|&id| id != 0) {
                let (name, hash) = self.names[id as usize - 1];
                let slot = self.slot(name, hash);
                self.slots[slot] = id;
            }
        }
        self.names.len() as u32 - 1
    }

    /// Returns the slot that holds `name`, or the empty slot where it belongs.
    fn slot(&self, name: &[u8], hash: u64) -> usize {
        let mask = self.slots.len() - 1;
        let mut slot = (hash >> 32) as usize & mask;
        while let Some(id) = self.slots[slot].checked_sub(1) {
            let (known, known_hash) = self.names[id as usize];
            if known_hash == hash && known == name {
                break;
            }
            slot = (slot + 1) & mask;
        }
        slot
    }
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
                .map(|&(name, hash)| all.intern(name, hash))
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

/// Computes and formats the output lines of each hour in `hours`, one buffer per hour.
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
) -> Vec<(usize, Vec<u8>)> {
    let mut done = Vec::new();
    // After the counting pass, endpoint e's latencies go to latencies[start[e]..start[e + 1]].
    let mut start = vec![0u32; names.len() + 1];
    let mut next = vec![0u32; names.len()];
    let mut errors = vec![0u32; names.len()];
    let mut latencies = Vec::new();
    for hour in hours {
        let mut out = Vec::new();
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
        let label = hour_label(hour);
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
            let errors = u64::from(errors[e]);
            let error_ppm = errors * 1_000_000 / n as u64;
            out.extend_from_slice(label.as_bytes());
            out.push(b' ');
            out.extend_from_slice(name);
            for value in [
                n as u64,
                p50.into(),
                p95.into(),
                p99.into(),
                errors,
                error_ppm,
            ] {
                push_number(&mut out, value);
            }
            out.push(b'\n');
        }
        done.push((hour, out));
    }
    done
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
