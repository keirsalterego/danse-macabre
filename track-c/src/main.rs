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
//! 3. **Each thread runs a scalar byte state machine.** [`aggregate`] walks its records one byte
//!    at a time and sums into its own [`Group`] per country, so the hot loop shares nothing but
//!    the name table. [`text_field`] measures a field as its raw length minus one per `""` pair,
//!    so no field is ever unescaped.
//! 4. **Distinct names go through one shared interner.** All threads intern names in one
//!    lock-free [`NameTable`]. Each distinct name's raw, still escaped bytes are copied once into
//!    an arena entry that also holds a bitset with one bit per country, so a record's compare
//!    and its bit land on the same cache line instead of a random line of the 200 MB file.
//!    Escaping is one to one, so two raw names differ exactly when their unescaped values do,
//!    and the quoted empty name `""` and the unquoted one are both the empty slice. Twelve
//!    per-thread tables would each hold most of the ~100k names, so a single shared table keeps
//!    one copy instead of twelve.
//! 5. **Merge, count and print.** The per-thread [`Group`]s hold only integers, so merging them
//!    gives the same result in any order. [`NameTable::distinct_per_group`] counts each
//!    country's bits, and [`format_output`] writes the lines.
//!
//! # What the code relies on
//!
//! The parser implements the grammar of spec §2.1 as written: a BOM at the start of any record,
//! CRLF or LF, a final record without a terminator, and unquoted text that is not empty. Its
//! output never depends on the narrower generator guarantees of §2.2; the sizes in §5 only set
//! the name table's starting size. It does rely on §2.3, where country codes are `[A-Z]{3}`
//! (checked while parsing), and on §3, where `id_sum` and `amount_sum` fit in 64 bits.
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
    /// libc's `mmap(2)`. The standard library already links libc, so declaring the one function
    /// we need costs no dependency.
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
}
/// `mmap` protection: the pages are readable and nothing else.
const PROT_READ: i32 = 1;
/// `mmap` flag: a private mapping, so nothing we do could ever write back to the file.
const MAP_PRIVATE: i32 = 2;

/// Reads `<data_dir>/regions.csv` and `<data_dir>/records.csv` and writes
/// `<out_dir>/aggregates.txt`. The directories come from the first two arguments and default to
/// `/data` and `/out`, where the harness mounts them.
fn main() -> Result<(), Box<dyn Error>> {
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

/// Builds [`Regions`] from `regions.csv` in two passes over its rows.
///
/// The first pass marks which countries occur and finds the largest id; the second fills the
/// array, which can only be sized once that id is known. Numbering the countries in between,
/// by walking the [`CODES`] table in index order, sorts them bytewise for free. regions.csv is
/// plain (spec §2.3), so rows split at LF with no quoting rules.
fn parse_regions(csv: &[u8]) -> Result<Regions, String> {
    let body = (csv.strip_prefix(b"region_id,country\n"))
        .ok_or("regions.csv: the header is not `region_id,country`")?;
    let rows = body.split(|&b| b == b'\n').filter(|row| !row.is_empty());
    let mut group_of_code = vec![0u16; CODES];
    let mut max_id = 0;
    for row in rows.clone() {
        let (id, code) = region_row(row)?;
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
    let mut group_of = vec![0; max_id + 1];
    for row in rows {
        let (id, code) = region_row(row)?;
        group_of[id] = group_of_code[code];
    }
    Ok(Regions { group_of, codes })
}

/// Parses one `region_id,country` row into the id and the country code as a base-26 number.
///
/// This is the one place the program checks its input. A row that breaks spec §2.3 returns an
/// error naming the row, because a bad code here would silently move every record of a region.
fn region_row(row: &[u8]) -> Result<(usize, usize), String> {
    let bad = || format!("regions.csv: bad row {:?}", String::from_utf8_lossy(row));
    let comma = row.iter().position(|&b| b == b',').ok_or_else(bad)?;
    let (id, code) = (&row[..comma], &row[comma + 1..]);
    if id.is_empty() || !id.iter().all(u8::is_ascii_digit) {
        return Err(bad());
    }
    if code.len() != 3 || !code.iter().all(u8::is_ascii_uppercase) {
        return Err(bad());
    }
    let id = id.iter().fold(0, |n, &d| n * 10 + usize::from(d - b'0'));
    let code = code.iter().fold(0, |n, &c| n * 26 + usize::from(c - b'A'));
    Ok((id, code))
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
            return Ok(format_output(&groups, &names.distinct_per_group(), regions));
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
    }
}

/// Parses the whole records in `data[range]` into one [`Group`] per country and marks each
/// record's name in `names`. Returns `None` when the name table is full.
///
/// This is the hot loop: a scalar state machine that reads each byte once, in order. The fields
/// come in a fixed order, so the state is simply the position in the record. The numbers parse as
/// they are scanned, and [`text_field`] skips names and notes while counting their escapes.
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
        groups[group].merge(&Group {
            count: 1,
            id_sum: id,
            amount_sum: cents,
            amount_min: cents,
            amount_max: cents,
            name_bytes,
            note_bytes,
        });
        names.mark(&data[name], group)?;
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
        if data[end] == b'"' {
            if data[end + 1] != b'"' {
                break;
            }
            pairs += 1;
            end += 2;
        } else {
            end += 1;
        }
    }
    (start..end, (end - start - pairs) as u64, end + 2)
}

/// The set of distinct names per country, shared by every thread without locks.
///
/// It is a string interner built as an insert-only, lock-free hash table with open addressing
/// and linear probing. The first time any thread sees a name, it writes the name into one
/// entry of a shared arena: its length, a bitset with one bit per group, and a copy of its bytes.
/// A record then sets its group's bit with an atomic OR. A set union does not depend on which
/// thread got there first, so the counts are deterministic.
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

    /// Adds `group` to the groups of `name`. Returns `None` when the table is full.
    fn mark(&self, name: &[u8], group: usize) -> Option<()> {
        let entry = self.intern(name)?;
        self.arena[entry + 1 + group / 64].fetch_or(1 << (group % 64), Relaxed);
        Some(())
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
                // A thread that loses the race leaves its entry unpublished: the entry's group
                // bits stay 0, so it counts for nothing.
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

    /// Per group, the number of names whose bit for that group is set, found by walking the
    /// arena entry by entry. It runs after every thread has joined, so relaxed loads see every
    /// bit, and every entry in the arena is complete.
    fn distinct_per_group(&self) -> Vec<u64> {
        let mut distinct = vec![0; self.words * 64];
        let (mut entry, used) = (0, self.arena_used.load(Relaxed));
        while entry < used {
            let len = self.arena[entry].load(Relaxed) as usize;
            let bitset = &self.arena[entry + 1..entry + 1 + self.words];
            for (w, word) in bitset.iter().enumerate() {
                let mut bits = word.load(Relaxed);
                // Visit only the set bits: take the lowest, then clear it.
                while bits != 0 {
                    distinct[w * 64 + bits.trailing_zeros() as usize] += 1;
                    bits &= bits - 1;
                }
            }
            entry += 1 + self.words + len.div_ceil(8);
        }
        distinct
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
fn format_output(groups: &[Group], distinct: &[u64], regions: &Regions) -> Vec<u8> {
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
            distinct[g],
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
