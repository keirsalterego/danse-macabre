//! Track C: join a hostile CSV of records against a region lookup table and print one line of
//! aggregates per country.
//!
//! # The input
//!
//! `records.csv` holds about 200 MB of records with five fields each,
//! `id,name,note,region_id,amount`. In the spec's notation (⟨BOM⟩ is `EF BB BF`, ␍␊ is CRLF and
//! ␊ is a bare LF), the start of a file looks like this:
//!
//! ```text
//! ⟨BOM⟩id,name,note,region_id,amount␍␊
//! 1,"Kaphar, Titus","said ""no"" twice",7,1420.00␍␊
//! 2,"Betts","a note that␊spans two lines",7,88.50␊
//! ⟨BOM⟩3,,"",12,0.00␍␊
//! ```
//!
//! Names and notes can be quoted, and inside the quotes a comma is data, `""` stands for one `"`,
//! and in notes a bare LF is data too. A record ends in CRLF or LF, and a byte order mark can sit
//! at the start of any record. `regions.csv` maps each `region_id` to a three-letter country
//! code. Every output line holds a country's record count, id sum, amount sum, min and max, its
//! number of distinct names, and the byte lengths of its names and notes after unescaping.
//!
//! # The pipeline
//!
//! 1. **The lookup becomes a direct-address table.** [`parse_regions`] parses `regions.csv` on
//!    every thread into a flat array indexed by region id that holds a group number, one byte
//!    per id. Countries are numbered in bytewise order, so the group number is already the output
//!    order and no sort is needed at the end. Group 0 is `_UNMATCHED`, and it is also what every
//!    id missing from the table reads as. The spec allows up to 26³ countries, more than a byte
//!    holds, so groups past 254 read as 255 and are found in a hash map instead (see
//!    [`Regions`]).
//! 2. **The records are split even though fields contain newlines.** The file is cut into one
//!    chunk per hardware thread, and a cut can land inside a quoted note, where an LF is data
//!    and not a record end. [`guess_bounds`] takes the first LF after each cut that a digit or a
//!    BOM follows, which no generated note holds, so no pass over the file comes first. Each
//!    thread reports where its run's last record ended, and the guesses are right exactly when
//!    every run ends where the next one starts. Otherwise [`record_bounds`] finds the exact cuts
//!    with a parallel pre-pass that takes the parity of each chunk's quotes, and the join runs
//!    again.
//! 3. **Each thread walks its records field by field.** [`aggregate`] parses each record in
//!    order and sums into its own [`Group`] per country. Names and notes are quoted, and
//!    [`text_field`] jumps from quote to quote 32 bytes at a time with AVX2 compares. It measures
//!    a field as its raw length minus one per `""` pair, so no field is ever unescaped.
//!    [`number`] reads each number from one 8-byte word, with no loop over its digits.
//! 4. **Each distinct name belongs to one thread.** The name's hash picks its owner, and each
//!    thread keeps the names it owns in a [`NameSet`] that no other thread touches: a hash table
//!    whose entry for a name holds the name's bytes and a bitset with one bit per country, on
//!    one cache line. A record's name usually comes from another thread's chunk, so the threads
//!    trade names in rounds. In each round a thread parses about 64 KiB of its records and posts
//!    each record's name and group to the name's owner as a 32-byte letter (see
//!    [`name_letter`]). All threads then meet at a barrier, and each reads the letters posted to
//!    it and sets the bits (see [`Exchange`]). A bit that goes from 0 to 1 is a name new to that
//!    country, so the counts come out of the marking with no pass over the tables afterwards.
//!
//!    Names are compared still escaped. Escaping is one to one, so two raw names differ exactly
//!    when their unescaped values do, and the quoted empty name `""` and the unquoted one are
//!    both the empty slice.
//!
//!    This replaced one lock-free table that all threads shared. There, each record's lookup
//!    missed the cache on a line that other cores also wrote, and set its bit with an atomic OR.
//!    Here only its owner touches a table, so none of its lines move between cores, and the only
//!    atomic read-modify-write is one per thread per round, at the barrier. The letters do move
//!    between cores, but they are written and read in order, two to a cache line. What the
//!    rounds cost instead is the wait at each barrier for the slowest thread (PERF.md, step 9).
//! 5. **Merge and print.** The per-thread [`Group`]s hold only integers, so merging them gives
//!    the same result in any order, and [`format_output`] writes the lines.
//!
//! # Memory
//!
//! The input is mapped and never copied (see [`map_file`]). What the program allocates is
//! mostly the name sets, about 6.4 MB of entries (one 64-byte line for each of ~100k names) and
//! 0.8 MB of slots at full scale, then about 1.2 MB of mailbox pages that the rounds touch and
//! the 1 MB array from region id to group. Every buffer of 64 KiB or more gets its own mapping,
//! so a freed buffer returns its pages at once, and transparent huge pages are off (see
//! [`main`]).
//!
//! # What the code relies on
//!
//! The parser implements the grammar of spec §2.1 as written: a BOM at the start of any record,
//! CRLF or LF, a final record without a terminator, and unquoted text that is not empty. Its
//! output never depends on the narrower generator guarantees of §2.2; the name pool of §2.2 and
//! the sizes in §5 only set the name sets' starting sizes. It does rely on §2.3, where country
//! codes are `[A-Z]{3}` (checked while parsing), and on §3, where `id_sum` and `amount_sum` fit
//! in 64 bits.
//! Records outside the §2.1 grammar are not validated.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use std::thread::{self, Thread};

/// The UTF-8 byte order mark, U+FEFF. At the start of the file or of a record it is skipped, and
/// anywhere else it is data (spec §2.1).
const BOM: &[u8] = b"\xEF\xBB\xBF";
/// The first line of `records.csv`, after its optional BOM.
const HEADER: &[u8] = b"id,name,note,region_id,amount";
/// The number of possible country codes. Codes are `[A-Z]{3}` (spec §2.3), so a code read as a
/// base-26 number indexes a table of 26³ entries, and that number sorts like the code's bytes.
const CODES: usize = 26 * 26 * 26;

unsafe extern "C" {
    /// libc's `mmap(2)`. The standard library already links libc, so declaring the functions
    /// we need costs no dependency.
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    /// libc's `prctl(2)`, used once, with [`PR_SET_THP_DISABLE`]. Like the C prototype it is
    /// variadic, and libc reads each extra argument as an `unsigned long`.
    fn prctl(option: i32, ...) -> i32;
    /// glibc's `mallopt(3)`, used once, with [`M_MMAP_THRESHOLD`].
    fn mallopt(param: i32, value: i32) -> i32;
    /// libc's `madvise(2)`, used with [`MADV_DONTNEED`] (see [`release`]).
    fn madvise(addr: *const u8, len: usize, advice: i32) -> i32;
}
/// `madvise` advice: drop the pages now. A later read of a file mapping maps the file again.
const MADV_DONTNEED: i32 = 4;
/// `mallopt` parameter: the size from which `malloc` maps memory of its own.
const M_MMAP_THRESHOLD: i32 = -3;
/// `mmap` protection: the pages are readable and nothing else.
const PROT_READ: i32 = 1;
/// `mmap` flag: a private mapping, so nothing we do could ever write back to the file.
const MAP_PRIVATE: i32 = 2;
/// `prctl` option: no transparent huge pages for this process (see [`main`]).
const PR_SET_THP_DISABLE: i32 = 41;

/// Reads `<data_dir>/regions.csv` and `<data_dir>/records.csv` and writes
/// `<out_dir>/aggregates.txt`. The directories come from the first two arguments and default to
/// `/data` and `/out`, where the harness mounts them.
fn main() -> Result<(), Box<dyn Error>> {
    // Transparent huge pages could charge a whole 2 MB page to a buffer that uses a few 4 KB
    // pages of it, such as the mailboxes, of which the rounds touch about half. When all
    // threads shared one name table, threads racing to fault on its fresh 2 MB regions added
    // 22 MiB to a 15 MiB peak (PERF.md, step 3). The private name sets measured the same peak
    // with THP on or off on a THP "always" machine, so this is insurance for a host that would
    // hand out huge pages more eagerly. If the call fails, only that is lost, so its result is
    // not checked.
    // SAFETY: the call takes plain integers and only changes this process's page-size policy.
    unsafe { prctl(PR_SET_THP_DISABLE, 1u64, 0u64, 0u64, 0u64) };
    // Every buffer of 64 KiB or more gets its own mapping, so a grown or freed one returns its
    // pages at once. By default glibc raises this threshold to the size of the largest buffer
    // freed so far. After the regions parse freed its 2 MB scratch array, every worker's name
    // set came from a malloc arena and grew there by copying, and the old copies stayed
    // resident: a 20 MiB peak instead of 14.
    // SAFETY: the call takes plain integers and only changes malloc's policy.
    unsafe { mallopt(M_MMAP_THRESHOLD, 64 << 10) };
    let mut args = std::env::args().skip(1);
    let data_dir = args.next().unwrap_or_else(|| "/data".into());
    let out_dir = args.next().unwrap_or_else(|| "/out".into());
    let regions = parse_regions(map_file(&Path::new(&data_dir).join("regions.csv"))?)?;
    let records = map_file(&Path::new(&data_dir).join("records.csv"))?;
    let out = join(records, &regions)?;
    let out_path = Path::new(&out_dir).join("aggregates.txt");
    std::fs::write(&out_path, out).map_err(|e| format!("{}: {e}", out_path.display()))?;
    // Exit without running destructors: freeing the name table would only add to the timed
    // wall. `fs::write` keeps no user-space buffer, so every byte is already written.
    std::process::exit(0)
}

/// Maps the whole file read-only and hands it out as one byte slice for the rest of the run.
///
/// The page cache is pre-warmed, so the mapping copies nothing and charges almost nothing to our
/// memory cgroup, where reading the file into the heap would charge all of it. Every name and
/// note stays a range into this slice and is never copied out.
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

/// Gives the whole pages inside `bytes` back to the kernel with `madvise(MADV_DONTNEED)`.
///
/// At exit the kernel unmaps every page of the input on one thread, inside the timed window: an
/// explicit `munmap` of the 200 MB took 2.8 ms. So each worker releases its own run of records,
/// in parallel, once the last round has shown that every worker is done parsing (see
/// [`aggregate`]). Releasing earlier would slow the threads still parsing, because dropping
/// pages makes the kernel flush every thread's TLB. A page shared with a neighbouring run stays
/// mapped. A later read of a dropped page maps the file's data again, so the call is only
/// advice, and its result is ignored.
fn release(bytes: &[u8]) {
    let start = bytes.as_ptr().addr();
    let (first, end) = (start.next_multiple_of(4096), (start + bytes.len()) & !4095);
    if first < end {
        // SAFETY: the pages lie inside `bytes`, in a read-only file mapping that stays mapped.
        unsafe { madvise(bytes.as_ptr().with_addr(first), end - first, MADV_DONTNEED) };
    }
}

/// The lookup table from `regions.csv`, as a direct-address table: the region id is the index.
///
/// Group 0 is `_UNMATCHED`, and country k in bytewise order is group k + 1. There are at most
/// 26³ codes (see [`CODES`]), so a `u16` holds every group, but a byte holds the groups of the
/// spec's 240 countries and halves the table, 1 MB at full scale.
struct Regions {
    /// Region id → group, sized from the largest id in the file. Ids it lacks stay 0, which is
    /// `_UNMATCHED`. The groups past 254 read as 255 here and are looked up in `wide`.
    group_of: Vec<u8>,
    /// The groups past 254, by region id.
    wide: HashMap<usize, u16>,
    /// Country codes in bytewise order: `codes[k]` is group k + 1.
    codes: Vec<[u8; 3]>,
}

impl Regions {
    /// The group of the region with this id; ids past the table's largest are `_UNMATCHED`.
    fn group(&self, id: usize) -> usize {
        match self.group_of.get(id) {
            Some(255) => usize::from(self.wide[&id]),
            Some(&group) => usize::from(group),
            None => 0,
        }
    }
}

/// Builds [`Regions`] from `regions.csv`, with its rows parsed on every hardware thread.
///
/// A row holds no quotes, so every LF ends one. Each thread takes the rows that start in one
/// even share of the file and lists them as (id, country code). Numbering the countries that
/// occur, by walking the [`CODES`] table in index order, then sorts them bytewise for free, and
/// one walk over the rows writes each region's group, narrowed to a byte. The join cannot start
/// before this ends, and on one thread it took 3.8 ms (PERF.md, step 11).
fn parse_regions(csv: &[u8]) -> Result<Regions, String> {
    let body = (csv.strip_prefix(b"region_id,country\n"))
        .ok_or("regions.csv: the header is not `region_id,country`")?;
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    // The first row that starts at or after the `i`th of `threads` even cuts.
    let cut = |i| match body.len() * i / threads {
        0 => 0,
        at => (body[at - 1..].iter().position(|&b| b == b'\n')).map_or(body.len(), |lf| at + lf),
    };
    let chunks = thread::scope(|s| {
        let parsers: Vec<_> = (0..threads)
            .map(|i| s.spawn(move || region_rows(body, cut(i)..cut(i + 1))))
            .collect();
        (parsers.into_iter())
            .map(|p| p.join().expect("a panic aborts the process"))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let (mut group_of_code, mut max_id) = (vec![0u16; CODES], 0);
    for &(id, code) in chunks.iter().flatten() {
        group_of_code[code] = 1;
        max_id = max_id.max(id);
    }
    let mut codes = Vec::new();
    for (code, group) in group_of_code.iter_mut().enumerate() {
        if *group != 0 {
            let letter = |place: usize| b'A' + (code / place % 26) as u8;
            codes.push([letter(26 * 26), letter(26), letter(1)]);
            *group = codes.len() as u16;
        }
    }
    let (mut group_of, mut wide) = (vec![0; max_id + 1], HashMap::new());
    for &(id, code) in chunks.iter().flatten() {
        let group = group_of_code[code];
        group_of[id] = group.min(255) as u8;
        if group >= 255 {
            wide.insert(id, group);
        }
    }
    Ok(Regions {
        group_of,
        wide,
        codes,
    })
}

/// Parses the rows that start in `rows`, each `region_id,country` and an LF (spec §2.3), into
/// pairs of the id and the country code as a base-26 number.
///
/// The digits run up to the comma, and the rest of a row has a fixed length, so the row's end
/// comes out of the parse with no search for the LF. This is the one place the program checks its
/// input. A row that breaks spec §2.3 returns an error naming the row, because a bad code here
/// would silently move every record of a region.
fn region_rows(body: &[u8], rows: Range<usize>) -> Result<Vec<(usize, usize)>, String> {
    // Sized for the shortest row, `0,ABC` and its LF, so that the list never grows: lists that
    // grew left their old copies resident in each thread's malloc arena, 1.5 MiB in all.
    let mut list = Vec::with_capacity(rows.len() / 6);
    let mut start = rows.start;
    while start < rows.end {
        let (mut at, mut id) = (start, 0);
        while let Some(&digit @ b'0'..=b'9') = body.get(at) {
            id = id * 10 + usize::from(digit - b'0');
            at += 1;
        }
        if let Some(&[b',', a, b, c, b'\n']) = body.get(at..at + 5)
            && at > start
            && [a, b, c].iter().all(u8::is_ascii_uppercase)
        {
            list.push((
                id,
                ([a, b, c].iter()).fold(0, |n, &c| n * 26 + usize::from(c - b'A')),
            ));
            start = at + 5;
        } else {
            let row = body[start..].split(|&b| b == b'\n').next();
            return Err(format!(
                "regions.csv: bad row {:?}",
                String::from_utf8_lossy(row.unwrap_or_default())
            ));
        }
    }
    Ok(list)
}

/// Bytes of its chunk a worker parses per round before the workers swap names. Small rounds
/// keep the mailboxes small and the names just parsed still in cache when their owner reads them.
const ROUND_BYTES: usize = 64 << 10;
/// Names one worker can post to another in one round. A round ends early when a mailbox fills.
// shortcut: the mailboxes take 2 × threads² × 8 KiB of address space, 2.25 MiB on 12 threads but
// 64 MiB on 64; size them from ROUND_BYTES / threads if this ever runs on many more cores.
const MAILBOX: usize = 256;
/// Spins of a worker that waits at the barrier before it parks (see [`Exchange::wait`]).
const SPINS: usize = 2000;

/// Parses every record, joins it to its group and returns the output text.
///
/// This is a parallel map-reduce. Each thread aggregates one run of whole records in
/// [`aggregate`], and the partial [`Group`]s are merged in thread order at the end. The runs
/// come from [`guess_bounds`], or from [`record_bounds`] when a guess turns out wrong. Distinct
/// names are split between the threads by hash, so each name has one owner that counts it, and
/// the threads hand names to their owners through an [`Exchange`].
fn join(records: &'static [u8], regions: &Regions) -> Result<Vec<u8>, String> {
    let body = body_start(records)?;
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let mut guess = guess_bounds(records, body, threads);
    loop {
        let guessed = guess.is_some();
        let bounds = (guess.take()).unwrap_or_else(|| record_bounds(records, body, threads));
        // A worker registers with its exchange once, so every attempt needs a new one.
        let exchange = &Exchange::new(threads);
        let parts: Vec<(Vec<Group>, usize)> = thread::scope(|s| {
            let work = |me, range| aggregate(me, records, range, regions, exchange);
            let workers: Vec<_> = (bounds.windows(2).enumerate())
                .map(|(me, w)| s.spawn(move || work(me, w[0]..w[1])))
                .collect();
            (workers.into_iter())
                .map(|w| w.join().expect("a panic aborts the process"))
                .collect()
        });
        // A guess is right exactly when the run before it ends where it starts. After a wrong
        // one the join runs again on exact runs.
        if guessed && !parts.iter().map(|p| p.1).eq(bounds[1..].iter().copied()) {
            continue;
        }
        let mut groups = vec![Group::default(); regions.codes.len() + 1];
        for (part, _) in &parts {
            groups.iter_mut().zip(part).for_each(|(g, p)| g.merge(p));
        }
        return Ok(format_output(&groups, regions));
    }
}

/// Returns the offset of the first record, past the optional BOM, the header and its CRLF or LF
/// (spec §2.1). A wrong header is an error, because it means the columns may be in another
/// order.
fn body_start(records: &[u8]) -> Result<usize, String> {
    let rest = records.strip_prefix(BOM).unwrap_or(records);
    let rest = (rest.strip_prefix(HEADER))
        .ok_or("records.csv: the header is not `id,name,note,region_id,amount`")?;
    let rest = rest.strip_prefix(b"\r").unwrap_or(rest);
    let rest = (rest.strip_prefix(b"\n")).ok_or("records.csv: no line break after the header")?;
    Ok(records.len() - rest.len())
}

/// Guesses where `parts` runs of whole records after `body` start, with no pass over the file.
/// Returns the starts and then the exact start of the file's last few records, which the last
/// run's worker parses after its run (see [`aggregate`]), or `None` if no tail fits the rules
/// below.
///
/// [`record_bounds`] reads all 200 MB once just to count quotes before any worker starts, and
/// dropping that pass saved 4 to 5 ms (PERF.md, step 10). A guess reads a few bytes per cut
/// instead: it takes the first LF that a digit or a BOM follows. A record starts that way, and an
/// LF inside a generated note is followed by a word or a quote (§2.2), so on generated data no
/// guess is wrong. Spec §2.1 lets any byte follow an LF in a note, though, so [`join`] checks every
/// guess after the parse and reruns on exact runs.
///
/// A wrong guess must cost only time, never a crash. The parse that starts at one reads past its
/// run, but never past the first real record whose name and note are quoted and hold more than
/// quotes. Such a field opens and closes with an odd run of quotes, and a quoted scan ends at the
/// first odd run, so each scan of a misread record stops at that record's quotes, commas or dot.
/// So the runs end at the start of the tail, which must hold such a record, and the tail itself
/// is parsed from its exact start. That start comes from the parity of the quotes after it: a
/// valid file holds an even number of quotes, so that parity is also the quote state where the
/// count starts.
fn guess_bounds(data: &[u8], body: usize, parts: usize) -> Option<Vec<usize>> {
    // 4 KiB hold dozens of records, and counting their quotes takes a microsecond.
    let from = data.len().saturating_sub(4 << 10).max(body);
    let odd = data[from..].iter().fold(false, |odd, &b| odd ^ (b == b'"'));
    let tail = next_record_start(data, from, odd);
    // A quoted field that holds more than quotes opens and closes with an odd run of quotes.
    let good = |f: Range<usize>| data[f.start - 1] == b'"' && data[f].iter().any(|&b| b != b'"');
    let mut at = tail;
    loop {
        let comma = at + data[at..].iter().position(|&b| b == b',')?;
        let (name, _, after_name) = text_field(data, comma + 1);
        let (note, _, after_note) = text_field(data, after_name);
        if good(name) && good(note) {
            break;
        }
        at = next_record_start(data, after_note, false);
    }
    // After an LF, a digit or a BOM, whose first byte is 0xEF. The first guess is `body` itself.
    let start = |&k: &usize| data[k - 1] == b'\n' && matches!(data[k], b'0'..=b'9' | 0xEF);
    let guess = |i| {
        let cut = body + (tail - body) * i / parts;
        (cut..tail).find(start).unwrap_or(tail)
    };
    Some((0..parts).map(guess).chain([tail]).collect())
}

/// Cuts the records after `body` into `parts` runs of whole records, with a quote-parity
/// pre-pass. It is exact on every valid file, and [`join`] falls back to it when a guess from
/// [`guess_bounds`] is wrong.
///
/// Splitting a CSV in parallel is hard because an LF inside a quoted field looks exactly like a
/// record end. The way out is that quotes come in pairs: unquoted fields hold none, and a quoted
/// field adds its two delimiters plus two per `""`. So a position is inside quotes exactly when
/// an odd number of quotes precede it. Each thread takes the parity of its chunk's quotes, and
/// a prefix XOR over those parities gives the quote state at every cut. [`next_record_start`]
/// then finds each cut's first record. The parity is an XOR of bytes, which the compiler
/// vectorizes over all 32 byte lanes of an AVX2 register (`vpcmpeqb` on `ymm`).
fn record_bounds(data: &[u8], body: usize, parts: usize) -> Vec<usize> {
    let cuts: Vec<usize> = (0..=parts)
        .map(|i| body + (data.len() - body) * i / parts)
        .collect();
    let odd_quotes: Vec<bool> = thread::scope(|s| {
        let counters: Vec<_> = (cuts.windows(2))
            .map(|w| &data[w[0]..w[1]])
            .map(|chunk| s.spawn(move || chunk.iter().fold(0, |odd, &b| odd ^ u8::from(b == b'"'))))
            .collect();
        (counters.into_iter())
            .map(|c| c.join().expect("a panic aborts the process") == 1)
            .collect()
    });
    let mut bounds = vec![body];
    let mut in_quotes = false;
    for (&cut, &odd) in cuts[1..parts].iter().zip(&odd_quotes) {
        in_quotes ^= odd;
        bounds.push(next_record_start(data, cut, in_quotes));
    }
    bounds.push(data.len());
    bounds
}

/// Returns the first record start at or after `at`, given `in_quotes`, the quote state at `at`.
///
/// A record starts right after an LF that is outside quotes (spec §2.1). A BOM, if any, is the
/// record's first byte, so it stays with the record. The scan starts one byte early so that `at`
/// itself counts when an LF precedes it, which keeps the bounds exact when a cut lands right on a
/// record start.
fn next_record_start(data: &[u8], at: usize, mut in_quotes: bool) -> usize {
    in_quotes ^= data[at - 1] == b'"';
    for (i, &b) in data[at - 1..].iter().enumerate() {
        match b {
            b'"' => in_quotes = !in_quotes,
            b'\n' if !in_quotes => return at + i,
            _ => {}
        }
    }
    data.len()
}

/// The running aggregates of one output line.
///
/// Every field is a sum, a min or a max, so groups merge in any order to the same result (see
/// [`Group::merge`]). Min and max start at the extremes, so an empty group merges as a no-op; a
/// max that started at 0 would print `0.00` for a country whose amounts are all negative.
#[derive(Clone, Copy)]
struct Group {
    count: u64,
    id_sum: u64,
    /// Money is integer cents throughout; no floating point is involved anywhere (spec §4).
    amount_sum: i64,
    amount_min: i64,
    amount_max: i64,
    name_bytes: u64,
    note_bytes: u64,
    /// Distinct names of the country among the names this thread owns. Each name has one owner,
    /// which counts it for a country once (see [`NameSet::read`]), so the merged sum is the
    /// country's number of distinct names.
    distinct: u64,
}

impl Default for Group {
    /// An empty group: no records, with min and max at the extremes.
    fn default() -> Self {
        Self {
            count: 0,
            id_sum: 0,
            amount_sum: 0,
            amount_min: i64::MAX,
            amount_max: i64::MIN,
            name_bytes: 0,
            note_bytes: 0,
            distinct: 0,
        }
    }
}

impl Group {
    /// Adds `other`'s records to this group. [`aggregate`] also adds each single record this
    /// way, as a group of one, so there is one place that knows how fields combine.
    fn merge(&mut self, other: &Self) {
        self.count += other.count;
        self.id_sum += other.id_sum;
        self.amount_sum += other.amount_sum;
        self.amount_min = self.amount_min.min(other.amount_min);
        self.amount_max = self.amount_max.max(other.amount_max);
        self.name_bytes += other.name_bytes;
        self.note_bytes += other.note_bytes;
        self.distinct += other.distinct;
    }
}

/// Parses the whole records in `data[range]` into one [`Group`] per country, and counts the
/// distinct names of the hash share that worker `me` owns.
///
/// This is the hot loop. The fields come in a fixed order, so the state is simply the position in
/// the record. [`number`] parses each number from one word, and [`text_field`] skips names and
/// notes while counting their escapes.
///
/// The worker runs in rounds. In each round it parses about [`ROUND_BYTES`] of its records and
/// posts each record's name to the worker that owns it (see [`Exchange`]), waits at the barrier
/// for all workers, and then marks the names posted to it in its own [`NameSet`].
///
/// Returns the groups and where the parse of `range` stopped, the first record start at or
/// after `range.end`, by which [`join`] checks guessed runs. The last worker then parses on from
/// `range.end` to the end of the file, because with runs from [`guess_bounds`] that is where
/// the file's tail starts. After the last round the worker gives its run's pages back (see
/// [`release`]).
fn aggregate(
    me: usize,
    data: &'static [u8],
    range: Range<usize>,
    regions: &Regions,
    exchange: &Exchange,
) -> (Vec<Group>, usize) {
    let group_count = regions.codes.len() + 1;
    let mut groups = vec![Group::default(); group_count];
    // Full scale draws from 100,000 names (spec §2.2) in 202 MB (§5), about one per 2 KiB, and
    // a thread owns a share of them. The set grows when a file has more.
    let mut names = NameSet::new(group_count, data.len() / 2048 / exchange.threads);
    let mut sent = vec![0; exchange.threads];
    let mut at = range.start;
    let (mut end, mut ended) = (range.end, None);
    (exchange.workers[me].set(thread::current())).expect("each worker registers once");
    for round in 0.. {
        let half = round % 2;
        sent.fill(0);
        let round_end = end.min(at + ROUND_BYTES);
        while at < round_end {
            // A BOM at the start of a record is not data (§2.1). It sits in front of the id, so
            // missing it would show up in `id_sum`.
            if data[at..].starts_with(BOM) {
                at += BOM.len();
            }
            let (id, comma) = number(data, at, b',');
            let (name, name_bytes, after_name) = text_field(data, comma + 1);
            let mut letter = name_letter(data, name);
            let (_, note_bytes, after_note) = text_field(data, after_name);
            let (region_id, comma) = number(data, after_note, b',');
            // The sign applies to the whole value, so -0.75 is -75 cents, not -0 * 100 + 75.
            let negative = data[comma + 1] == b'-';
            let (whole, dot) = number(data, comma + 1 + usize::from(negative), b'.');
            let fraction = u64::from(data[dot + 1] - b'0') * 10 + u64::from(data[dot + 2] - b'0');
            let cents = (whole * 100 + fraction).cast_signed();
            let cents = if negative { -cents } else { cents };
            // Past the CR if any, then the LF. Only the last record may end at end of file instead
            // (§2.1), which leaves `at` past the end and stops the loop.
            at = dot + 4 + usize::from(data.get(dot + 3) == Some(&b'\r'));
            // Records also reference ids past the table's largest (§2.2), so the lookup is
            // bounds-checked; those ids are unmatched like the holes inside the table.
            let group = regions.group(region_id as usize);
            letter[3] |= (group as u64) << 48;
            let to = exchange.owner(letter[3] as u32);
            exchange.post(half, me, to, sent[to], letter);
            sent[to] += 1;
            groups[group].merge(&Group {
                count: 1,
                id_sum: id,
                amount_sum: cents,
                amount_min: cents,
                amount_max: cents,
                name_bytes,
                note_bytes,
                distinct: 0,
            });
            if sent[to] == MAILBOX {
                break;
            }
        }
        // The last worker goes on from the exact start of the file's tail (see `guess_bounds`).
        if at >= end && ended.is_none() && me + 1 == exchange.threads {
            (ended, at, end) = (Some(at), end, data.len());
        }
        exchange.seal(half, me, &sent);
        let all_done = exchange.wait(me, round, at >= end);
        for from in 0..exchange.threads {
            names.read(exchange.letters(half, from, me), data, &mut groups);
        }
        if all_done {
            break;
        }
    }
    release(&data[range]);
    (groups, ended.unwrap_or(at))
}

/// Parses the decimal digits from `at` up to the `stop` byte, and returns the value and the
/// position of `stop`. The field's end and its value come out of the same pass.
///
/// A field whose `stop` lies within 8 bytes is parsed from one 8-byte word, with no loop and no
/// branch on its length (SWAR: SIMD within a register). XOR with `stop` in every byte turns each
/// `stop` into a zero byte, which a zero-byte test finds. Shifting the digits to the top of the
/// word turns the missing ones into leading zeros, and three multiplies then join neighbouring
/// digits into pairs, pairs into fours and fours into the value. Longer fields, such as an id
/// with leading zeros (§2.1 allows them), and the file's last 7 bytes take the loop over bytes.
///
/// It is always inlined. At each of its three call sites `stop` is a constant, so the call and
/// the multiply that copies `stop` into every byte disappear, 29 instructions per record.
#[inline(always)]
fn number(data: &[u8], mut at: usize, stop: u8) -> (u64, usize) {
    const ONES: u64 = u64::from_le_bytes([1; 8]);
    if let Some(&chunk) = data[at..].first_chunk() {
        let word = u64::from_le_bytes(chunk);
        let x = word ^ (ONES * u64::from(stop));
        // The lowest set bit marks the first zero byte of `x`, so the first `stop`. Bits above
        // it can be false, which is why only the lowest one is used.
        let stops = x.wrapping_sub(ONES) & !x & (ONES << 7);
        if stops != 0 {
            let len = stops.trailing_zeros() as usize / 8;
            // In two steps, because a 0-digit field would need a shift by 64.
            let digits = ((word & (ONES * 0x0F)) << (56 - 8 * len)) << 8;
            let pairs = (digits.wrapping_mul(10 << 8 | 1) >> 8) & 0x00FF_00FF_00FF_00FF;
            let fours = (pairs.wrapping_mul(100 << 16 | 1) >> 16) & 0x0000_FFFF_0000_FFFF;
            return (fours.wrapping_mul(10000 << 32 | 1) >> 32, at + len);
        }
    }
    let mut value = 0;
    while data[at] != stop {
        value = value * 10 + u64::from(data[at] - b'0');
        at += 1;
    }
    (value, at)
}

/// Reads the name or note field at `at`, which a comma ends. Returns the field's raw bytes
/// (between the quotes, still escaped), the value's length after unescaping, and the position
/// after the comma.
///
/// Unescaping would copy every field. The spec's byte counts need only the length, and that is
/// the raw length minus one per `""` pair. Distinct counting needs no unescaping either (see
/// [`NameSet`]).
///
/// Inside quotes only the quotes matter, so the scan jumps from one quote to the next with
/// [`byte_mask`], 32 bytes per step. A name or a note usually ends within the first step, where
/// a loop over bytes took one step and one branch per byte. The file's last 31 bytes, too few for
/// a step, go one byte at a time.
///
/// It is always inlined. As a call it returned its four words through the stack, and the caller
/// read two of them back with one 16-byte load, which waits until both 8-byte stores reach the
/// cache because store forwarding cannot merge them.
#[inline(always)]
fn text_field(data: &[u8], at: usize) -> (Range<usize>, u64, usize) {
    if data[at] != b'"' {
        // The generator writes unquoted text only empty (§2.2), but §2.1 allows any bytes but
        // quote, comma, CR and LF.
        let mut end = at;
        while data[end] != b',' {
            end += 1;
        }
        return (at..end, (end - at) as u64, end + 1);
    }
    let start = at + 1;
    let (mut end, mut pairs) = (start, 0);
    // Inside quotes only `"` is special: `""` is one escaped quote, and any other `"` closes the
    // field. Never test for "a quote followed by a comma": a note can hold `"",` (an escaped
    // quote, then a comma), which that test would take for the end of the field.
    // A note can also start or end with an escaped quote, so the field can open or close with
    // `"""`; pairing quotes left to right reads both right.
    loop {
        // On to the next quote. An empty mask has 32 trailing zeros, so a step that finds no
        // quote moves on by all 32 bytes.
        end += match data[end..].first_chunk() {
            Some(chunk) => byte_mask(chunk, b'"').trailing_zeros() as usize,
            None => usize::from(data[end] != b'"'),
        };
        if data[end] != b'"' {
            continue;
        }
        if data[end + 1] != b'"' {
            break;
        }
        pairs += 1;
        end += 2;
    }
    (start..end, (end - start - pairs) as u64, end + 2)
}

/// Returns a mask with bit i set where `chunk[i] == byte`.
///
/// The compiler turns the loop into one AVX2 compare of all 32 bytes (`vpcmpeqb` on `ymm`) and
/// one `vpmovmskb` that packs the 32 results into the mask, so finding the next quote takes a
/// handful of instructions however far away it is within the 32 bytes.
fn byte_mask(chunk: &[u8; 32], byte: u8) -> u32 {
    let mut mask = 0;
    for (i, &b) in chunk.iter().enumerate() {
        mask |= u32::from(b == byte) << i;
    }
    mask
}

/// The mailboxes through which the workers hand each name to the worker that owns it, and the
/// barrier that separates posting from reading.
///
/// There are two sets of mailboxes, one per parity of the round. In round r a worker posts into
/// set r % 2, which nobody reads at that time: the set was last read in round r - 2, and every
/// worker finished reading it before it arrived at the barrier of round r - 1, which the poster
/// has passed (see [`Exchange::wait`]). So one barrier per round is enough. The letters are
/// atomics only so that safe code can share them; every access is `Relaxed`, a plain load or
/// store, and the barrier orders them.
struct Exchange {
    threads: usize,
    /// `[half][from][to]` mailboxes of [`MAILBOX`] letters of four words each (see
    /// [`name_letter`]).
    letters: Box<[AtomicU64]>,
    /// How many letters each `[half][from][to]` mailbox holds this round.
    counts: Box<[AtomicU64]>,
    /// Workers at the barrier in the low 32 bits, and how many of them have finished their
    /// chunks in the high 32 bits.
    arrived: AtomicU64,
    /// Twice the number of barriers passed, plus 1 if every worker had finished at the last one.
    released: AtomicU64,
    /// Each worker's thread, so that the last one to reach a barrier can unpark the others.
    workers: Box<[OnceLock<Thread>]>,
}

impl Exchange {
    fn new(threads: usize) -> Self {
        Self {
            threads,
            letters: zeroed(2 * threads * threads * MAILBOX * 4),
            counts: zeroed(2 * threads * threads),
            arrived: AtomicU64::new(0),
            released: AtomicU64::new(0),
            workers: (0..threads).map(|_| OnceLock::new()).collect(),
        }
    }

    /// The worker that owns names with this hash: the hash scaled to `0..threads`.
    fn owner(&self, hash: u32) -> usize {
        ((u64::from(hash) * self.threads as u64) >> 32) as usize
    }

    fn mailbox(&self, half: usize, from: usize, to: usize) -> usize {
        (half * self.threads + from) * self.threads + to
    }

    /// Writes letter `index` of the mailbox from `from` to `to`.
    fn post(&self, half: usize, from: usize, to: usize, index: usize, letter: [u64; 4]) {
        let at = (self.mailbox(half, from, to) * MAILBOX + index) * 4;
        (self.letters[at..at + 4].iter())
            .zip(letter)
            .for_each(|(w, v)| w.store(v, Relaxed));
    }

    /// Publishes how many letters `from` posted to each worker this round.
    fn seal(&self, half: usize, from: usize, sent: &[usize]) {
        for (to, &count) in sent.iter().enumerate() {
            self.counts[self.mailbox(half, from, to)].store(count as u64, Relaxed);
        }
    }

    /// The letters `from` posted to `to` this round, four words each.
    fn letters(&self, half: usize, from: usize, to: usize) -> &[AtomicU64] {
        let mailbox = self.mailbox(half, from, to);
        let count = self.counts[mailbox].load(Relaxed) as usize;
        &self.letters[mailbox * MAILBOX * 4..][..count * 4]
    }

    /// Waits until every worker has reached the barrier of `round`, and returns whether every
    /// worker had `done` its chunk by then. All workers get the same answer, so they all stop
    /// after the same round.
    ///
    /// The last worker to arrive resets the count and releases the others. Its `AcqRel` add
    /// sees every earlier arrival's writes, and its `Release` store passes them on to the
    /// `Acquire` loads of the waiters, so every letter posted before the barrier is visible after.
    fn wait(&self, me: usize, round: usize, done: bool) -> bool {
        let arrival = 1 + (u64::from(done) << 32);
        let arrived = self.arrived.fetch_add(arrival, AcqRel) + arrival;
        let next = 2 * (round as u64 + 1);
        if arrived as u32 as usize == self.threads {
            self.arrived.store(0, Relaxed);
            let all_done = (arrived >> 32) as usize == self.threads;
            self.released.store(next + u64::from(all_done), Release);
            (self.workers.iter().enumerate())
                .filter(|&(worker, _)| worker != me)
                .for_each(|(_, thread)| thread.get().expect("set before round 0").unpark());
            return all_done;
        }
        for spins in 0usize.. {
            let released = self.released.load(Acquire);
            if released >= next {
                return released & 1 == 1;
            }
            // Workers usually arrive within microseconds of each other, so a waiter spins
            // first. A worker that is late because the scheduler put it aside, or put two workers
            // on one CPU, would keep every spinner busy for whole time slices, and busy CPUs never
            // look idle enough for the scheduler to move it. So a waiter that has spun long
            // parks, which frees its CPU, and the last worker to arrive unparks everyone.
            if spins < SPINS {
                std::hint::spin_loop();
            } else {
                thread::park();
            }
        }
        unreachable!("the loop above only returns")
    }
}

/// Names of up to this many bytes travel inside their letter and are keyed by their bytes.
const INLINE: usize = 24;
/// The length field of a letter whose name is longer than [`INLINE`] bytes.
const LONG: u64 = 0xFFFF;
/// The bits of a letter's last word that belong to the key: the hash and the length.
const KEY: u64 = (1 << 48) - 1;

/// A letter for the name at `start..end` of `data`, without its group, which the poster adds to
/// the top 16 bits: the name's bytes as three little-endian words zero-padded to [`INLINE`]
/// bytes, then its 32-bit hash with its length above it. A name longer than that travels as its
/// start and end offsets instead, with [`LONG`] as length.
///
/// The three words come from one fixed-size read past the name's start, masked to its length,
/// so short names need no variable-length copy. The bytes after a name are the rest of its
/// record, so the read stays in the file except for the last few bytes.
///
/// The hash is `FxHash`, the rotate-XOR-multiply hash rustc uses for its own tables, with a
/// final fold of the high half into the low half, so the 32 bits kept depend on every word. The
/// length seeds it, so names that differ only by trailing spaces, such as `Betts` and `Betts `,
/// differ from the first step. For a long name it covers the first [`INLINE`] bytes, which is
/// enough to pick an owner, and the owner compares long names in full.
fn name_letter(data: &[u8], Range { start, end }: Range<usize>) -> [u64; 4] {
    let len = end - start;
    let mut bytes = [0; INLINE];
    match data.get(start..start + INLINE) {
        Some(window) => bytes.copy_from_slice(window),
        // Only a name shorter than the window can end this close to the end of the file.
        None => bytes[..len].copy_from_slice(&data[start..end]),
    }
    let words: [u64; 3] = std::array::from_fn(|i| {
        let keep = len.saturating_sub(8 * i).min(8);
        u64::from_le_bytes(bytes.as_chunks().0[i]) & ((1u128 << (8 * keep)) - 1) as u64
    });
    let mix = |h: u64, w: u64| (h.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    let h = words.iter().fold(len as u64, |h, &w| mix(h, w));
    let hash = (h ^ (h >> 32)) & 0xFFFF_FFFF;
    if len > INLINE {
        return [start as u64, end as u64, 0, hash | LONG << 32];
    }
    [words[0], words[1], words[2], hash | (len as u64) << 32]
}

/// The low 24 bits of a slot: its entry's index plus 1, so that 0 is an empty slot. The top 8
/// bits are bits 20 to 27 of the name's hash, so a probe that passes a different name usually
/// skips its entry without loading it.
const ENTRY: u32 = (1 << 24) - 1;

/// One cache line of a [`NameSet`] entry.
#[derive(Clone, Copy)]
#[repr(align(64))]
struct Line([u64; 8]);

/// One worker's share of the distinct names, with the countries each name occurs in.
///
/// It is a hash table with open addressing and linear probing that only its worker ever
/// touches: no atomics and no cache line shared with another core. Each distinct name has one
/// entry of whole cache lines: its four-word key from [`name_letter`], then a bitset with one bit
/// per group. With up to 256 groups an entry is one line, so a record's compare and its bit land
/// on the same line. A slot holds an entry's number and 8 more bits of the name's hash (see
/// [`ENTRY`]), so an empty slot costs 4 bytes, and a probe loads an entry only when those bits
/// match. Names longer than [`INLINE`] bytes are rare. Their keys say where they lie in the file,
/// and two of them are compared there (see [`NameSet::holds`]).
///
/// Names are keyed still escaped. Escaping is one to one (each `"` becomes `""`), so two raw
/// names are equal exactly when their unescaped values are. The empty quoted name `""` and the
/// empty unquoted name both arrive as an empty slice, so they are one name, as spec §3 asks.
struct NameSet {
    /// The entry of each name, with 8 bits of its hash on top (see [`ENTRY`]), or 0.
    slots: Vec<u32>,
    /// The entries, `stride` lines each.
    lines: Vec<Line>,
    /// Lines per entry: four key words, then one bit per group.
    stride: usize,
}

impl NameSet {
    /// An empty set with room for about `names` names over `group_count` groups.
    fn new(group_count: usize, names: usize) -> Self {
        let stride = (4 + group_count.div_ceil(64)).div_ceil(8);
        Self {
            // The slots may fill to three quarters before they grow (see `insert`).
            slots: vec![0; (4 * names / 3).next_power_of_two().max(16)],
            // Pages of the capacity that no entry reaches cost nothing, so this is generous.
            lines: Vec::with_capacity(2 * names * stride),
            stride,
        }
    }

    /// Marks the names of `letters` and counts each name that is new for its group in
    /// `groups`. Kept out of line so that this loop gets the registers to itself.
    #[inline(never)]
    fn read(&mut self, letters: &[AtomicU64], data: &[u8], groups: &mut [Group]) {
        for letter in letters.as_chunks::<4>().0 {
            let letter: [u64; 4] = std::array::from_fn(|i| letter[i].load(Relaxed));
            let group = (letter[3] >> 48) as usize;
            let key = [letter[0], letter[1], letter[2], letter[3] & KEY];
            groups[group].distinct += u64::from(self.mark(key, group, data));
        }
    }

    /// Adds `group` to the groups of the name with this key and returns `true` if it was not
    /// there yet.
    fn mark(&mut self, key: [u64; 4], group: usize, data: &[u8]) -> bool {
        let mask = self.slots.len() - 1;
        let mut slot = key[3] as u32 as usize & mask;
        let tag = (key[3] as u32 >> 20) << 24;
        let entry = loop {
            let found = self.slots[slot];
            match (found & ENTRY) as usize {
                0 => break self.insert(slot, tag, key),
                e if found & !ENTRY == tag && self.holds(e - 1, key, data) => break e - 1,
                _ => slot = (slot + 1) & mask,
            }
        };
        let word = 4 + group / 64;
        let word = &mut self.lines[entry * self.stride + word / 8].0[word % 8];
        let bit = 1 << (group % 64);
        let new = *word & bit == 0;
        *word |= bit;
        new
    }

    /// Whether `entry` is the name of `key`. A short name's key is the whole name, so equal keys
    /// are equal names. A long name's key says where the name lies in `data`, so two long names
    /// with the same hash are compared there.
    fn holds(&self, entry: usize, key: [u64; 4], data: &[u8]) -> bool {
        let held = self.key(entry);
        let text = |key: &[u64]| &data[key[0] as usize..key[1] as usize];
        held == key || held[3] == key[3] && key[3] >> 32 == LONG && text(held) == text(&key)
    }

    /// Appends an entry with this key and no groups, puts it in the empty `slot` with its `tag`,
    /// and returns its index. The slots grow when the entries fill three quarters of them; see
    /// [`NameSet::grow`].
    fn insert(&mut self, slot: usize, tag: u32, key: [u64; 4]) -> usize {
        let entry = self.lines.len() / self.stride;
        assert!(entry < ENTRY as usize, "under 2^24 names per thread");
        self.lines.resize((entry + 1) * self.stride, Line([0; 8]));
        self.lines[entry * self.stride].0[..4].copy_from_slice(&key);
        self.slots[slot] = tag | (entry + 1) as u32;
        if 4 * (entry + 1) > 3 * self.slots.len() {
            self.grow();
        }
        entry
    }

    /// Doubles the slots and places every entry again, tag and all, so at least a quarter of the
    /// slots stay empty and every probe ends at an empty slot.
    fn grow(&mut self) {
        let mut slots = vec![0; 2 * self.slots.len()];
        let mask = slots.len() - 1;
        for &found in self.slots.iter().filter(|&&found| found != 0) {
            let mut slot = self.key((found & ENTRY) as usize - 1)[3] as u32 as usize & mask;
            while slots[slot] != 0 {
                slot = (slot + 1) & mask;
            }
            slots[slot] = found;
        }
        self.slots = slots;
    }

    /// The key that `entry` starts with.
    fn key(&self, entry: usize) -> &[u64] {
        &self.lines[entry * self.stride].0[..4]
    }
}

/// Allocates `len` atomics that are all 0.
///
/// The memory comes zeroed from the allocator, which for big sizes means fresh pages from the
/// kernel that are mapped in only when first written. So the mailbox pages that no round fills
/// cost no memory, and the mailboxes can be sized generously.
fn zeroed(len: usize) -> Box<[AtomicU64]> {
    // SAFETY: `AtomicU64` has the same layout as `u64`, and all-zero bits are a valid 0.
    unsafe { Box::new_zeroed_slice(len).assume_init() }
}

/// Writes one line per group that has records: the countries in bytewise order, then
/// `_UNMATCHED`, which sorts after every capital letter because `_` is 0x5F (spec §4).
///
/// Group numbers already follow bytewise order (see [`Regions`]), so this is a walk, not a sort.
fn format_output(groups: &[Group], regions: &Regions) -> Vec<u8> {
    let mut out = Vec::new();
    for g in (1..groups.len()).chain([0]) {
        let group = &groups[g];
        if group.count == 0 {
            continue; // a country without records prints nothing (§3)
        }
        let key: &[u8] = if g == 0 {
            b"_UNMATCHED"
        } else {
            &regions.codes[g - 1]
        };
        out.extend_from_slice(key);
        writeln!(
            out,
            " {} {} {} {} {} {} {} {}",
            group.count,
            group.id_sum,
            Cents(group.amount_sum),
            Cents(group.amount_min),
            Cents(group.amount_max),
            group.distinct,
            group.name_bytes,
            group.note_bytes
        )
        .expect("writing to a Vec cannot fail");
    }
    out
}

/// Integer cents that display as money, `-?I.DD`: -525 is `-5.25`, -5 is `-0.05`, and zero is
/// `0.00`, never `-0.00` (spec §4).
struct Cents(i64);

impl fmt::Display for Cents {
    /// Splits the magnitude into whole units and two digits. The sign is printed only for a
    /// negative value, which is what rules out `-0.00`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        write!(f, "{sign}{}.{:02}", abs / 100, abs % 100)
    }
}
