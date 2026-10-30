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
//! 1. **The lookup becomes a direct-address table.** [`parse_regions`] turns `regions.csv` into a
//!    flat array indexed by region id that holds a group number. Countries are numbered in
//!    bytewise order, so the group number is already the output order and no sort is needed at
//!    the end. Group 0 is `_UNMATCHED`, and it is also what every id missing from the table
//!    reads as.
//! 2. **The records are split even though fields contain newlines.** The file is cut into one
//!    chunk per hardware thread, and a cut can land inside a quoted note, where an LF is data
//!    and not a record end. [`record_bounds`] runs a parallel pre-pass that takes the parity of
//!    each chunk's quotes. A prefix XOR over those parities tells every cut whether it is inside
//!    quotes, and [`next_record_start`] then walks to the first real record. No thread has to
//!    parse the chunks before its own.
//! 3. **Each thread walks its records field by field.** [`aggregate`] parses each record in
//!    order and sums into its own [`Group`] per country, so the hot loop shares nothing but the
//!    name table. Names and notes are quoted, and [`text_field`] jumps from quote to quote 32
//!    bytes at a time with AVX2 compares. It measures a field as its raw length minus one per
//!    `""` pair, so no field is ever unescaped.
//! 4. **Distinct names go through one shared interner.** All threads intern names in one
//!    lock-free [`NameTable`]. Each distinct name's raw, still escaped bytes are copied once into
//!    an arena entry that also holds a bitset with one bit per country, so a record's compare
//!    and its bit land on the same cache line instead of a random line of the 200 MB file.
//!    Escaping is one to one, so two raw names differ exactly when their unescaped values do,
//!    and the quoted empty name `""` and the unquoted one are both the empty slice. Twelve
//!    per-thread tables would each hold most of the ~100k names, so a single shared table keeps
//!    one copy instead of twelve.
//!
//!    The atomic OR that sets a bit returns the word it replaced, so exactly one record sees each
//!    name's bit for a country go from 0 to 1, and its thread counts that name as new for the
//!    country. The counts need no pass over the table afterwards.
//! 5. **Merge and print.** The per-thread [`Group`]s hold only integers, so merging them gives
//!    the same result in any order, and [`format_output`] writes the lines.
//!
//! # Memory
//!
//! The input is mapped and never copied (see [`map_file`]). What the program allocates is
//! mostly the name table, 4 MB of slots and about 5 MB of arena in use at full scale, and the
//! 2 MB array from region id to group. Transparent huge pages are off, because twelve threads
//! faulting on the same fresh 2 MB region at once more than doubled the peak (see [`main`]).
//!
//! # What the code relies on
//!
//! The parser implements the grammar of spec §2.1 as written: a BOM at the start of any record,
//! CRLF or LF, a final record without a terminator, and unquoted text that is not empty. Its
//! output never depends on the narrower generator guarantees of §2.2; the sizes in §5 only set
//! the name table's starting size. It does rely on §2.3, where country codes are `[A-Z]{3}` and
//! region ids ascend, so the last one is the largest (both checked while parsing), and on §3,
//! where `id_sum` and `amount_sum` fit in 64 bits.
//! Records outside the §2.1 grammar are not validated.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::thread;

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
}
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
    // Transparent huge pages cost memory here and buy little speed. The name table's arena
    // grows at one front that every thread writes. When the front reaches a fresh 2 MB region,
    // each thread that faults on it before the first fault finishes allocates and zeroes a
    // 2 MB page of its own, and memory.peak counts them all until the losers free theirs. On
    // 12 cores that added 22 MiB to a 15 MiB peak, while 4 KB pages cost 2% more CPU time and
    // under 1% more wall time. If the call fails, only that saving is lost, so its result is
    // not checked.
    // SAFETY: the call takes plain integers and only changes this process's page-size policy.
    unsafe { prctl(PR_SET_THP_DISABLE, 1u64, 0u64, 0u64, 0u64) };
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

/// The lookup table from `regions.csv`, as a direct-address table: the region id is the index.
///
/// Group 0 is `_UNMATCHED`, and country k in bytewise order is group k + 1. A `u16` holds every
/// group, because there are at most 26³ codes (see [`CODES`]).
struct Regions {
    /// Region id → group, sized from the largest id in the file. Ids it lacks stay 0, which is
    /// `_UNMATCHED`.
    group_of: Vec<u16>,
    /// Country codes in bytewise order: `codes[k]` is group k + 1.
    codes: Vec<[u8; 3]>,
}

/// Builds [`Regions`] from `regions.csv` in one pass over its rows.
///
/// Region ids ascend (spec §2.3), so the last row's id sizes the array before the pass. The pass
/// stores each region's country code and marks which countries occur. Numbering the countries
/// afterwards, by walking the [`CODES`] table in index order, sorts them bytewise for free, and
/// one walk over the array then swaps each code for its group. Two passes that each split the
/// rows at LF and then parsed them took three times as long.
fn parse_regions(csv: &[u8]) -> Result<Regions, String> {
    let body = (csv.strip_prefix(b"region_id,country\n"))
        .ok_or("regions.csv: the header is not `region_id,country`")?;
    let last_row = (body[..body.len().saturating_sub(1)].iter())
        .rposition(|&b| b == b'\n')
        .map_or(0, |lf| lf + 1);
    let max_id = if body.is_empty() {
        0
    } else {
        region_row(body, last_row)?.0
    };
    // Until the last walk, an entry holds its country's code + 1, so that 0 still means no
    // region. That is at most 26³, which fits a `u16`.
    let mut group_of = vec![0u16; max_id + 1];
    let mut group_of_code = vec![0u16; CODES];
    let mut at = 0;
    while at < body.len() {
        let (id, code, next) = region_row(body, at)?;
        *(group_of.get_mut(id)).ok_or("regions.csv: ids do not ascend")? = code as u16 + 1;
        group_of_code[code] = 1;
        at = next;
    }
    let mut codes = Vec::new();
    for (code, group) in group_of_code.iter_mut().enumerate() {
        if *group != 0 {
            let letter = |place: usize| b'A' + (code / place % 26) as u8;
            codes.push([letter(26 * 26), letter(26), letter(1)]);
            *group = codes.len() as u16;
        }
    }
    let group_of_entry: Vec<u16> = [0].into_iter().chain(group_of_code).collect();
    for group in &mut group_of {
        *group = group_of_entry[usize::from(*group)];
    }
    Ok(Regions { group_of, codes })
}

/// Parses the row that starts at `body[start]`, `region_id,country` and an LF (spec §2.3), into
/// the id, the country code as a base-26 number, and the start of the next row.
///
/// The digits run up to the comma, and the rest of the row has a fixed length, so the row's end
/// comes out of the parse with no search for the LF. This is the one place the program checks its
/// input. A row that breaks spec §2.3 returns an error naming the row, because a bad code here
/// would silently move every record of a region.
fn region_row(body: &[u8], start: usize) -> Result<(usize, usize, usize), String> {
    let mut at = start;
    let mut id = 0;
    while let Some(&digit @ b'0'..=b'9') = body.get(at) {
        id = id * 10 + usize::from(digit - b'0');
        at += 1;
    }
    if let Some(&[b',', a, b, c, b'\n']) = body.get(at..at + 5)
        && at > start
        && [a, b, c].iter().all(u8::is_ascii_uppercase)
    {
        let code = ([a, b, c].iter()).fold(0, |n, &c| n * 26 + usize::from(c - b'A'));
        return Ok((id, code, at + 5));
    }
    let row = body[start..].split(|&b| b == b'\n').next();
    Err(format!(
        "regions.csv: bad row {:?}",
        String::from_utf8_lossy(row.unwrap_or_default())
    ))
}

/// Parses every record, joins it to its group and returns the output text.
///
/// This is a parallel map-reduce. Each thread aggregates one run of whole records from
/// [`record_bounds`] in [`aggregate`], all threads share one [`NameTable`], and the partial
/// [`Group`]s are merged in thread order at the end.
fn join(records: &'static [u8], regions: &Regions) -> Result<Vec<u8>, String> {
    let body = body_start(records)?;
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let bounds = record_bounds(records, body, threads);
    let group_count = regions.codes.len() + 1;
    // Records average about 64 bytes (spec §5), and one slot per 8 records gives full scale
    // 2^19 slots for its ~100k distinct names. The table cannot grow while threads insert into
    // it, so a full table is rebuilt 4x larger and the join reruns: no input is too big for it,
    // only slower.
    let mut slot_count = (records.len() / 512).next_power_of_two();
    loop {
        let names = &NameTable::new(slot_count, group_count);
        let parts = thread::scope(|s| {
            let workers: Vec<_> = (bounds.windows(2))
                .map(|w| w[0]..w[1])
                .map(|range| s.spawn(move || aggregate(records, range, regions, names)))
                .collect();
            (workers.into_iter())
                .map(|w| w.join().expect("a panic aborts the process"))
                .collect::<Option<Vec<_>>>()
        });
        if let Some(parts) = parts {
            let mut groups = vec![Group::default(); group_count];
            for part in &parts {
                groups.iter_mut().zip(part).for_each(|(g, p)| g.merge(p));
            }
            return Ok(format_output(&groups, regions));
        }
        slot_count *= 4;
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

/// Cuts the records after `body` into `parts` runs of whole records, with a quote-parity
/// pre-pass.
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
    /// Names that one of these records added to the country's set of names. Across all threads
    /// each name is added to a country exactly once (see [`NameTable::mark`]), so the merged sum
    /// is the country's number of distinct names.
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

/// Parses the whole records in `data[range]` into one [`Group`] per country and marks each
/// record's name in `names`. Returns `None` when the name table is full.
///
/// This is the hot loop. The fields come in a fixed order, so the state is simply the position in
/// the record. The numbers parse as they are scanned, and [`text_field`] skips names and notes
/// while counting their escapes.
fn aggregate(
    data: &[u8],
    range: Range<usize>,
    regions: &Regions,
    names: &NameTable,
) -> Option<Vec<Group>> {
    let mut groups = vec![Group::default(); regions.codes.len() + 1];
    let mut at = range.start;
    while at < range.end {
        // A BOM at the start of a record is not data (§2.1). It sits in front of the id, so
        // missing it would show up in `id_sum`.
        if data[at..].starts_with(BOM) {
            at += BOM.len();
        }
        let (id, comma) = number(data, at, b',');
        let (name, name_bytes, after_name) = text_field(data, comma + 1);
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
        let group = (regions.group_of.get(region_id as usize)).map_or(0, |&g| usize::from(g));
        let new_name = names.mark(&data[name], group)?;
        groups[group].merge(&Group {
            count: 1,
            id_sum: id,
            amount_sum: cents,
            amount_min: cents,
            amount_max: cents,
            name_bytes,
            note_bytes,
            distinct: u64::from(new_name),
        });
    }
    Some(groups)
}

/// Parses the decimal digits from `at` up to the `stop` byte, and returns the value and the
/// position of `stop`. The field's end and its value come out of the same pass.
fn number(data: &[u8], mut at: usize, stop: u8) -> (u64, usize) {
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
/// [`NameTable`]).
///
/// Inside quotes only the quotes matter, so the scan jumps from one quote to the next with
/// [`byte_mask`], 32 bytes per step. A name or a note usually ends within the first step, where
/// a loop over bytes took one step and one branch per byte. The file's last 31 bytes, too few for
/// a step, go one byte at a time.
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

/// The set of distinct names per country, shared by every thread without locks.
///
/// It is a string interner built as an insert-only, lock-free hash table with open addressing
/// and linear probing. The first time any thread sees a name, it writes the name into one
/// entry of a shared arena: its length, a bitset with one bit per group, and a copy of its bytes.
/// A record then sets its group's bit with an atomic OR, which also says whether the bit was
/// already set. Which thread sets a bit first changes from run to run, but each bit is set first
/// exactly once, so the counts are deterministic.
///
/// The entry keeps everything a record needs side by side, so a record touches two places in
/// the table: its slot and its entry, where the compare and the OR hit the same cache line. The
/// copy of the bytes matters most. Comparing against the name's first occurrence in the 200 MB
/// file instead cost a miss to DRAM on nearly every record, about 30% of all cycles.
///
/// Names are interned still escaped. Escaping is one to one (each `"` becomes `""`), so two raw
/// names are equal exactly when their unescaped values are. The empty quoted name `""` and the
/// empty unquoted name both arrive as an empty slice, so they are one name, as spec §3 asks.
struct NameTable {
    /// The hash's high 32 bits above `entry + 1`, the word offset of the name's entry in
    /// `arena`, or 0 for an empty slot. Keeping a hash tag in the slot means a probe reads an
    /// entry only when the tags match.
    slots: Box<[AtomicU64]>,
    /// The entries, back to back. An entry is one word holding the name's length in bytes, then
    /// `words` bitset words, then the name in little-endian words (see [`words`]).
    arena: Box<[AtomicU64]>,
    /// The first free word of `arena`.
    arena_used: AtomicUsize,
    /// Entries written so far. They stop at half the slot count; see [`NameTable::insert`].
    names: AtomicUsize,
    /// Bitset words per entry: one bit per group, `_UNMATCHED` included.
    words: usize,
}

impl NameTable {
    /// An empty table with `slot_count` slots (a power of two) and room for `slot_count / 2`
    /// names, each with a bit for every one of `group_count` groups.
    fn new(slot_count: usize, group_count: usize) -> Self {
        let words = group_count.div_ceil(64);
        // Enough arena for names that average 32 bytes. Pages no entry reaches cost nothing
        // (see [`zeroed`]), and a full arena reruns the join just like a full slot table.
        let arena_words = slot_count / 2 * (1 + words + 4);
        assert!(
            arena_words < 1 << 32,
            "an entry offset must fit the low half of its slot"
        );
        Self {
            slots: zeroed(slot_count),
            arena: zeroed(arena_words),
            arena_used: AtomicUsize::new(0),
            names: AtomicUsize::new(0),
            words,
        }
    }

    /// Adds `group` to the groups of `name` and returns `true` if this call is the one that added
    /// it. Returns `None` when the table is full.
    ///
    /// The OR is one atomic read-modify-write, so of all the calls for one name and group, in
    /// any threads, exactly one sees the bit clear.
    fn mark(&self, name: &[u8], group: usize) -> Option<bool> {
        let entry = self.intern(name)?;
        let bit = 1 << (group % 64);
        Some(self.arena[entry + 1 + group / 64].fetch_or(bit, Relaxed) & bit == 0)
    }

    /// Returns the offset of `name`'s entry, inserting the name if it is new.
    ///
    /// A new entry is written in full first and then published with a compare-and-swap on the
    /// empty slot. Release on that swap and Acquire on every slot load mean that any thread that
    /// sees the slot also sees the whole entry.
    fn intern(&self, name: &[u8]) -> Option<usize> {
        const LOW: u64 = u32::MAX as u64;
        let hash = hash(name);
        let mask = self.slots.len() - 1;
        let mut slot = hash as usize & mask;
        loop {
            let mut seen = self.slots[slot].load(Acquire);
            if seen == 0 {
                let entry = self.insert(name)?;
                // A thread that loses the race leaves its entry unpublished. No record ever
                // marks it, so it counts for nothing.
                let new = hash & !LOW | (entry as u64 + 1);
                match self.slots[slot].compare_exchange(0, new, Release, Acquire) {
                    Ok(_) => return Some(entry),
                    Err(winner) => seen = winner,
                }
            }
            // A matching tag is not proof: names are always compared byte for byte.
            if seen & !LOW == hash & !LOW && self.holds((seen & LOW) as usize - 1, name) {
                return Some((seen & LOW) as usize - 1);
            }
            slot = (slot + 1) & mask;
        }
    }

    /// Writes a new, unpublished entry for `name` and returns its offset.
    ///
    /// Returns `None` once the arena is full or the entries reach half the slot count; then
    /// [`join`] reruns with a bigger table. The second limit keeps at least half the slots
    /// empty, so every probe in [`NameTable::intern`] ends at an empty slot.
    fn insert(&self, name: &[u8]) -> Option<usize> {
        let size = 1 + self.words + name.len().div_ceil(8);
        let entry = self.arena_used.fetch_add(size, Relaxed);
        if self.names.fetch_add(1, Relaxed) >= self.slots.len() / 2
            || entry + size > self.arena.len()
        {
            return None;
        }
        self.arena[entry].store(name.len() as u64, Relaxed);
        let text = &self.arena[entry + 1 + self.words..entry + size];
        text.iter()
            .zip(words(name))
            .for_each(|(a, w)| a.store(w, Relaxed));
        Some(entry)
    }

    /// Whether the entry at `entry` holds `name`: same length, then the same words.
    fn holds(&self, entry: usize, name: &[u8]) -> bool {
        let text = &self.arena[entry + 1 + self.words..];
        self.arena[entry].load(Relaxed) == name.len() as u64
            && words(name).zip(text).all(|(w, a)| a.load(Relaxed) == w)
    }
}

/// Allocates `len` atomics that are all 0.
///
/// The memory comes zeroed from the allocator, which for big sizes means fresh pages from the
/// kernel that are mapped in only when first written. So the parts of the [`NameTable`] that no
/// name reaches cost no memory, and the table can be sized generously.
fn zeroed(len: usize) -> Box<[AtomicU64]> {
    // SAFETY: `AtomicU64` has the same layout as `u64`, and all-zero bits are a valid 0.
    unsafe { Box::new_zeroed_slice(len).assume_init() }
}

/// A name as little-endian 8-byte words, the last one zero-padded: the form [`hash`] mixes and
/// the arena stores. Comparing words instead of bytes takes two steps for a typical 10-byte name.
fn words(name: &[u8]) -> impl Iterator<Item = u64> {
    let (whole, tail) = name.as_chunks::<8>();
    let mut last = [0; 8];
    last[..tail.len()].copy_from_slice(tail);
    let last = (!tail.is_empty()).then_some(u64::from_le_bytes(last));
    whole.iter().map(|w| u64::from_le_bytes(*w)).chain(last)
}

/// Hashes a name with `FxHash`, the rotate-XOR-multiply hash rustc uses for its own tables.
///
/// It takes eight bytes per step, and a final fold mixes the high half into the low half, so
/// the low bits (the slot) and the high bits (the tag) both depend on every byte. The length
/// seeds the hash, so names that differ only by trailing spaces, such as `Betts` and `Betts `,
/// differ from the first step.
fn hash(name: &[u8]) -> u64 {
    let mix = |h: u64, w: u64| (h.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    let h = words(name).fold(name.len() as u64, mix);
    h ^ (h >> 32)
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
