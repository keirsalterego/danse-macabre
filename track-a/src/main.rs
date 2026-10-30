//! Track A: per-(hour, endpoint) request counts, exact latency percentiles and error rates.
//!
//! Pipeline, on every hardware thread:
//! 1. mmap `requests.log` and cut it into one chunk per thread at line breaks.
//! 2. Parse each chunk once. Endpoints are interned per thread, and each line becomes one 8-byte
//!    record (endpoint id, hour, error bit, latency). The chunk's records are then sorted by hour
//!    in place.
//! 3. Merge the per-thread endpoint names and sort them bytewise: rank order is output order.
//! 4. Each thread takes a range of hours. Per hour it counts lines per endpoint, places the
//!    latencies endpoint by endpoint in one buffer that fits in L2, and selects the three
//!    nearest ranks with `select_nth_unstable` (exact, no full sort).
//!
//! The input is parsed once, not twice: at 239 MB it is ten times the L3, so a second pass would
//! stream it from DRAM again. The records cost 25 MB of memory instead. Numbers: PERF.md.

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::thread;

/// Every timestamp lies in the same 72 hours, 2026-10-30T18:00Z to 2026-11-02T17:59:59.999999Z,
/// for every seed and scale (spec §2, field `ts`).
const HOURS: usize = 72;
/// The shortest possible line: a 27-byte timestamp, a 3-byte method, a 1-byte endpoint, a 3-digit
/// status, a 1-digit latency, four spaces and the LF.
const MIN_LINE: usize = 40;

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
}
const PROT_READ: i32 = 1;
const MAP_PRIVATE: i32 = 2;

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
    // Exit without running destructors: freeing the record and output buffers would only add to
    // the timed wall. `File` keeps no user-space buffer, so every byte is already written.
    std::process::exit(0)
}

/// Maps the whole file read-only. The page cache is pre-warmed, so this copies nothing and
/// charges almost nothing to our memory cgroup, unlike reading it into the heap.
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

/// Returns the output in pieces, one per thread, to be written in order.
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
    let (chunks, ranks, names) = (&chunks, &ranks, &names);
    Ok(thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|t| HOURS * t / threads..HOURS * (t + 1) / threads)
            .map(|hours| s.spawn(move || summarize_hours(hours, chunks, ranks, names)))
            .collect();
        (workers.into_iter())
            .map(|w| w.join().expect("a panic aborts the process"))
            .collect()
    }))
}

/// Splits `log` at `parts - 1` line starts, so every line lands in exactly one chunk.
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
struct Chunk<'a> {
    /// Local endpoint id → (name, hash). Names borrow the mapped file.
    names: Vec<(&'a [u8], u64)>,
    /// One per line: `id << 40 | hour << 33 | error << 32 | latency_us`, sorted by hour.
    records: Vec<u64>,
    /// `records[hour_start[h]..hour_start[h + 1]]` are hour h's lines.
    hour_start: [usize; HOURS + 1],
}

impl Chunk<'_> {
    fn hour(&self, hour: usize) -> &[u64] {
        &self.records[self.hour_start[hour]..self.hour_start[hour + 1]]
    }
}

/// Parses one chunk of whole lines. The spec guarantees well-formed input (§2), so fields are
/// found by fixed offsets and single-byte checks rather than validated. No field is scanned
/// byte by byte: one AVX2 window finds the line end, and the latency, status and endpoint end
/// are read backwards from it.
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

/// Index of the LF that ends `line`. No line is shorter than `MIN_LINE`, so the search starts
/// at that line's last byte, and the first 64-byte window holds the LF for ~90% of lines.
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

/// Bit i is set where `block[i]` is an LF: two 32-byte AVX2 compares.
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

/// Hour index 0..72 from the timestamp's fixed offsets: month at 5, day at 8, hour at 11.
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

/// The latency that ends at `lf`, and the index of the space before it. Up to 7 digits (99.9%
/// of lines) take one branch-free step: find the last space in the 8 bytes before the LF, zero
/// every byte up to it, and convert the remaining digit lanes all at once.
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

/// Converts eight digit lanes (values 0..=9, most significant in the lowest byte; leading lanes
/// zeroed) to their value: pairs, then the halves of the number, in three multiplies instead of
/// eight (Lemire, "Fast numeric string to int", 2018).
fn eight_digits(lanes: u64) -> u32 {
    let pairs = lanes * 10 + (lanes >> 8);
    let high = (pairs & 0x0000_00FF_0000_00FF).wrapping_mul(100 + (1_000_000 << 32));
    let low = ((pairs >> 16) & 0x0000_00FF_0000_00FF).wrapping_mul(1 + (10_000 << 32));
    ((high + low) >> 32) as u32
}

/// Reads the 8 bytes at `at` as one little-endian word.
fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(*bytes[at..].first_chunk().expect("8 bytes at `at`"))
}

/// Sets bit 7 of exactly the bytes of `w` that are spaces. No borrow crosses bytes, so the
/// highest set bit is as trustworthy as the lowest.
fn spaces(w: u64) -> u64 {
    let x = w ^ 0x2020_2020_2020_2020;
    !(((x & 0x7F7F_7F7F_7F7F_7F7F) + 0x7F7F_7F7F_7F7F_7F7F) | x) & 0x8080_8080_8080_8080
}

/// Hashes an endpoint from its length and three 8-byte windows (first, middle, last), with no
/// loop. Every window lies inside the name, so equal names hash equal; different names that
/// share all three windows only cost an extra probe, since the interner compares bytes.
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

/// Open-addressing map from endpoint bytes to dense ids. It doubles at half load, so it assumes
/// nothing about how many distinct endpoints a seed has.
struct Interner<'a> {
    /// Id + 1 per slot; 0 marks an empty slot.
    slots: Vec<u32>,
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

    /// The slot holding `name`, or the empty slot where it belongs (linear probing).
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

/// In-place counting sort by hour (American flag sort): each record moves at most once, and a
/// chunk's records never leave that thread's own buffer.
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

/// Global ranks: the union of every chunk's names, sorted bytewise, so rank order is output
/// order (a proper prefix sorts first, as the spec asks). Returns the sorted names and, per
/// chunk, local id → rank.
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

/// Formats the output lines of `hours`, which arrive already in output order.
fn summarize_hours(
    hours: Range<usize>,
    chunks: &[Chunk],
    ranks: &[Vec<u32>],
    names: &[&[u8]],
) -> Vec<u8> {
    let mut out = Vec::new();
    // After the counting pass, endpoint e's latencies go to latencies[start[e]..start[e + 1]].
    let mut start = vec![0u32; names.len() + 1];
    let mut next = vec![0u32; names.len()];
    let mut errors = vec![0u32; names.len()];
    let mut latencies = Vec::new();
    for hour in hours {
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
            // Nearest rank, 1-indexed: ceil(p·n / 100), the spec's (p·n + 99) / 100 (§4).
            let index = |p: usize| (p * n).div_ceil(100) - 1;
            // Select the highest rank first: each lower rank then lies in the prefix it leaves.
            // The prefixes are inclusive because ranks coincide for small n (all three at n = 1).
            let p99 = *group.select_nth_unstable(index(99)).1;
            let p95 = *group[..=index(99)].select_nth_unstable(index(95)).1;
            let p50 = *group[..=index(95)].select_nth_unstable(index(50)).1;
            let errors = u64::from(errors[e]);
            let error_ppm = errors * 1_000_000 / n as u64;
            out.extend_from_slice(label.as_bytes());
            out.push(b' ');
            out.extend_from_slice(name);
            writeln!(out, " {n} {p50} {p95} {p99} {errors} {error_ppm}")
                .expect("writing to a Vec cannot fail");
        }
    }
    out
}

/// `YYYY-MM-DDTHH:00:00Z` for hour index `hour` of the span that starts 2026-10-30T18:00Z.
fn hour_label(hour: usize) -> String {
    let (day, hh) = ((hour + 18) / 24, (hour + 18) % 24);
    let (month, dd) = if day < 2 {
        (10, 30 + day)
    } else {
        (11, day - 1)
    };
    format!("2026-{month:02}-{dd:02}T{hh:02}:00:00Z")
}
