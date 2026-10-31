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
//! 1. **The lookup becomes a direct-address table.** The threads of [`join`] first parse
//!    `regions.csv` into a flat array indexed by region id that holds a group number, one byte
//!    per id. Countries are numbered in bytewise order, so the group number is already the output
//!    order and no sort is needed at the end. Group 0 is `_UNMATCHED`, and it is also what every
//!    id missing from the table reads as.
//! 2. **The records are split even though fields contain newlines.** The file is cut into one
//!    run per hardware thread, and a cut can land inside a quoted note, where an LF is data and
//!    not a record end. Each run starts at the first LF after its cut that a digit or a BOM
//!    follows (see [`join`]), so no pass over the file comes first.
//! 3. **Each thread walks its records field by field.** [`join`] parses each record in
//!    order and sums into its own [`Group`] per country. Names and notes are quoted, and
//!    [`text_fields`] finds the ends of both at once, from masks of the quotes and the commas in
//!    the 64 bytes at the name that AVX2 compares build. It measures a field as its raw length
//!    minus one per `""` pair, so no field is ever unescaped. [`number`] reads the id from one
//!    8-byte word, and [`region_amount`] the region id and the amount from one 16-byte vector,
//!    with no loop over their digits.
//! 4. **Each distinct name belongs to one thread.** The name's hash picks its owner, and each
//!    thread keeps the names it owns in a [`NameSet`] that no other thread touches: a hash table
//!    whose entry for a name holds 64 bits of the name's hash and a bitset with one bit per
//!    country. A record's name usually comes from another thread's run, so the threads trade
//!    names in rounds. In each round a thread parses 64 KiB of its records and posts each
//!    record's name and group to the name's owner as a 16-byte letter, the name's 128-bit hash
//!    with the group in its top byte (see [`name_letter`]). All threads then meet at a barrier,
//!    and each reads the letters posted to it and sets the bits (see [`join`]). A bit that goes
//!    from 0 to 1 is a name new to that country, so the counts come out of the marking with no
//!    pass over the tables afterwards.
//!
//!    Names are hashed still escaped. Escaping is one to one, so two raw names differ exactly
//!    when their unescaped values do, and the quoted empty name `""` and the unquoted one are
//!    both the empty slice. A name is known by its hash alone: two names count as one only if 72
//!    bits of their hashes agree, which a run's names do with odds of about 1 in 10^13.
//!
//!    This replaced one lock-free table that all threads shared. There, each record's lookup
//!    missed the cache on a line that other cores also wrote, and set its bit with an atomic OR.
//!    Here only its owner touches a table, so none of its lines move between cores, and the only
//!    atomic read-modify-write is one per thread per round, at the barrier. What the rounds cost
//!    instead is the wait at each barrier for the slowest thread (PERF.md, step 9).
//! 5. **Merge and print.** The per-thread [`Group`]s hold only integers, so merging them gives
//!    the same result in any order, and [`format_output`] writes the lines.
//!
//! # Memory
//!
//! The input is mapped and never copied (see [`map_file`]). What the program allocates is
//! mostly the name sets, about 4.0 MB of entries (40 bytes for each of ~100k names) and 0.8 MB
//! of slots at full scale, then about 1.2 MB of mailbox pages that the rounds touch and
//! the 1 MB array from region id to group. Every buffer of 64 KiB or more gets its own mapping,
//! so a freed buffer returns its pages at once, and transparent huge pages are off (see
//! [`main`]).
//!
//! # What the code relies on
//!
//! The parser implements the grammar of spec §2.1. It relies on §2.2 in two places: an LF inside
//! a note is followed by a lowercase word or a quote, never by a digit or a BOM, which is what
//! makes the run starts exact, and a name holds no NUL byte, so that its bytes padded with zeros
//! stand for it in its hash (see [`name_letter`]). It relies on §2.3, where country codes are
//! `[A-Z]{3}` and there are 240 countries (more than 255 stop the join with an error), and on §3,
//! where `id_sum` and `amount_sum` fit in 64 bits. The name pool of §2.2 and the sizes in §5 only
//! set the name sets' starting sizes, and §2.2's bounds on region ids and amounts only size the
//! window of [`region_amount`]: a field past them takes a slower path. Records outside the §2.1
//! grammar are not validated.

#![no_main]

use std::arch::x86_64::*;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering, Ordering::Relaxed};
use std::{fs::write, io::Write, ops::Range, os::fd::AsRawFd, path::Path, thread, time::Duration};

/// The UTF-8 byte order mark, U+FEFF. At the start of the file or of a record it is skipped, and
/// anywhere else it is data (spec §2.1).
const BOM: &[u8] = b"\xEF\xBB\xBF";
/// Zero bytes that [`map_file`] puts after the last byte of each file. Every position a parse
/// reaches lies before them, so the fixed-size reads of [`number`] (8 bytes), [`region_amount`]
/// (16), [`name_letter`] (32) and [`byte_mask`] (64) always fit in the input, and none of them
/// needs a shorter path for its end.
const PAD: usize = 64;
/// Bytes of its run a worker parses per round before the workers swap names. Small rounds keep
/// the mailboxes small and the names just parsed still in cache when their owner reads them.
const ROUND_BYTES: usize = 64 << 10;
/// Letters one worker can post to another in one round: one per record that starts in the
/// round, and a record takes at least 11 bytes (`1,,,1,0.00` and its LF), so no round fills a
/// mailbox. The pages of a mailbox that no round reaches are never touched, so they cost address
/// space only. The count is rounded up to whole 4 KiB pages, of 256 letters of 16 bytes, so that
/// each mailbox starts a page: a round posts about 90 letters to each owner, which then fill part
/// of one page, where mailboxes that started mid-page mostly touched two and peaked 1.7 MiB
/// higher.
const MAILBOX: usize = (ROUND_BYTES / 11 + 1).next_multiple_of(256);
/// Spins of a worker that waits at the barrier before it starts to sleep (see [`barrier`]).
const SPINS: usize = 2000;
/// The country codes. They are `[A-Z]{3}` (spec §2.3), so a code read as a base-26 number is
/// below 26³, and that number sorts like the code's bytes.
const CODES: usize = 26 * 26 * 26;
/// 32 bytes of ones, then 32 zeros. The 32 bytes from `32 - len` on are the mask that keeps the
/// first `len` bytes of a name (see [`name_letter`]).
const KEEP: [[u8; 32]; 2] = [[!0; 32], [0; 32]];
/// The round key of the name hash's last two rounds (see [`name_letter`]), the first 32 hex
/// digits of π's fraction. Any constant whose four 4-byte columns differ would do, and one key
/// serves both rounds: four keys took a table of nine lines and gained nothing for names that no
/// adversary chose.
// SAFETY: `u128` and `__m128i` are both 16 bytes, and every bit pattern is a valid one.
const KEY: __m128i = unsafe { std::mem::transmute(0x243F_6A88_85A3_08D3_1319_8A2E_0370_7344_u128) };
/// The largest entry number a slot holds. A slot's top 24 bits are its entry's index plus 1, so
/// that 0 is an empty slot, and its low 8 bits are the name's tag, byte 14 of its hash, so a probe
/// that passes a different name usually skips its entry without loading it. With the number on
/// top, one shift takes it out and the bits compare as a byte: with the bits on top, a mask for
/// each took one more instruction per probe and one more per letter.
const ENTRY: u32 = (1 << 24) - 1;

unsafe extern "C" {
    /// libc's `mmap(2)`. The standard library already links libc, so declaring the functions
    /// we need costs no dependency.
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    /// libc's `prctl(2)`, used once, with `PR_SET_THP_DISABLE`. Like the C prototype it is
    /// variadic, and libc reads each extra argument as an `unsigned long`.
    fn prctl(option: i32, ...) -> i32;
    /// glibc's `mallopt(3)`, used once, with `M_MMAP_THRESHOLD`.
    fn mallopt(param: i32, value: i32) -> i32;
    /// libc's `madvise(2)`, used with `MADV_POPULATE_WRITE` and `MADV_DONTNEED` (see [`join`]).
    fn madvise(addr: *const u8, len: usize, advice: i32) -> i32;
}

/// The running aggregates of one output line: the record count, the id sum, the amount sum, the
/// name bytes, the note bytes, the distinct names, the amount min and the amount max. Money is
/// integer cents throughout; no floating point is involved anywhere (spec §4).
///
/// The first six are sums and the last two a min and a max, so groups merge in any order to the
/// same result (see [`merge`]). Min and max start at the extremes, so an empty group merges as a
/// no-op; a max that started at 0 would print `0.00` for a country whose amounts are all
/// negative.
type Group = [i64; 8];
/// An empty group: no records, with min and max at the extremes.
const EMPTY: Group = [0, 0, 0, 0, 0, 0, i64::MAX, i64::MIN];

/// Reads `<data_dir>/regions.csv` and `<data_dir>/records.csv` and writes
/// `<out_dir>/aggregates.txt`. The directories come from the first two arguments and default to
/// `/data` and `/out`, where the harness mounts them.
///
/// This is C's `main`, called with no Rust runtime set up before it (`#![no_main]`). That setup
/// installs a handler for stack overflows, and with it every spawned thread maps and protects the
/// pages of a signal stack of its own, two calls that take the address space's lock while the
/// other threads start. Without it the runs took 0.1 to 0.3 ms less (PERF.md, step 38). The
/// arguments still come from `std::env::args`, which glibc hands to the standard library before
/// `main`.
#[unsafe(no_mangle)]
extern "C" fn main() -> ! {
    // Transparent huge pages would charge a whole 2 MB page to a buffer that uses a few 4 KB
    // pages of it, such as the mailboxes, of which the rounds touch a few percent. If the call
    // fails, only that is lost, so its result is not checked.
    // SAFETY: the call takes plain integers and only changes this process's page-size policy.
    unsafe { prctl(41, 1u64, 0u64, 0u64, 0u64) }; // PR_SET_THP_DISABLE
    // Every buffer of 64 KiB or more gets its own mapping, so a grown or freed one returns its
    // pages at once. By default glibc raises this threshold to the size of the largest buffer
    // freed so far, and after the regions parse freed its lists, every worker's name set came
    // from a malloc arena and grew there by copying: a 20 MiB peak instead of 14.
    // SAFETY: the call takes plain integers and only changes malloc's policy.
    unsafe { mallopt(-3, 64 << 10) }; // M_MMAP_THRESHOLD
    let data_dir = std::env::args().nth(1).unwrap_or("/data".into());
    let out = Path::new(&std::env::args().nth(2).unwrap_or("/out".into())).join("aggregates.txt");
    let threads = thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let file = |name| map_file(&Path::new(&data_dir).join(name)).expect("a readable input");
    join(file("records.csv"), file("regions.csv"), threads, &out)
}

/// Maps the whole file, followed by [`PAD`] zero bytes, and hands it out as one byte slice for
/// the rest of the run.
///
/// The page cache is pre-warmed, so the mapping copies nothing and charges almost nothing to our
/// memory cgroup, where reading the file into the heap would charge all of it. Every name and
/// note stays a range into this slice and is never copied out.
///
/// The file is mapped over the start of a reservation of anonymous memory, whose rest is the
/// pad. Anonymous memory reads as zeros, and so does the rest of the file's last page.
fn map_file(path: &Path) -> Result<&'static [u8], String> {
    let err = |e| format!("{e}: {}", path.display());
    let file = std::fs::File::open(path).map_err(err)?;
    let len = file.metadata().map_err(err)?.len() as usize;
    // SAFETY: fresh memory at an address the kernel picks overlaps no memory we already use.
    // PROT_READ (1), MAP_PRIVATE | MAP_ANONYMOUS (0x22).
    let at = unsafe { mmap(std::ptr::null_mut(), len + PAD, 1, 0x22, -1, 0) };
    // SAFETY: the file goes over the start of that fresh memory, which nothing refers to yet. If
    // the first call failed, this one fails too, at the address -1.
    // PROT_READ (1), MAP_PRIVATE | MAP_FIXED (0x12).
    let ptr = unsafe { mmap(at, len, 1, 0x12, file.as_raw_fd(), 0) };
    if ptr.addr() == usize::MAX {
        return Err(err(std::io::Error::last_os_error()));
    }
    // SAFETY: the mappings hold `len + PAD` readable bytes and are never unmapped, so they
    // outlive every borrow. /data is mounted read-only, so the file's bytes cannot change.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len + PAD) })
}

/// Calls `row` with (region id, code) for each row of `regions.csv` that starts in the given one
/// of `shares` even shares of the file, where the code is the country as a number below
/// [`CODES`].
///
/// A row holds no quotes, so every LF ends one. Each row is `region_id,country` and an LF (spec
/// §2.3): the digits run up to the comma, and the rest of a row has a fixed length, so the row's
/// end comes out of the parse with no search for the LF.
fn region_rows(csv: &[u8], share: usize, shares: usize, mut row: impl FnMut(usize, usize)) {
    // From the header's LF on, so that a row starts at `p` exactly when `body[p - 1]` is an LF.
    let body = &csv[csv.iter().position(|&b| b == b'\n').expect("a header line")..];
    let len = body.len() - PAD;
    // The first row that starts after the `i`th of `shares` even cuts.
    let cut = |i| ((len * i / shares + 1..len).find(|&p| body[p - 1] == b'\n')).unwrap_or(len);
    let (mut at, end) = (cut(share), cut(share + 1));
    while at < end {
        // SAFETY: `at` is below `end`, at most `len`, so the 8 bytes from it end inside the pad.
        let (id, comma) = unsafe { number(body, at, b',') };
        assert!(body[comma + 4] == b'\n', "regions.csv: bad row");
        let [a, b, c] = [1, 2, 3].map(|i| (body[comma + i] - b'A') as usize);
        row(id as usize, (a * 26 + b) * 26 + c);
        at = comma + 5;
    }
}

/// Parses every record of `data`, the file and its [`PAD`], joins it to its group by the
/// `regions` table, writes the lines of [`format_output`] to `out` and ends the process.
///
/// This is a parallel map-reduce. Each thread aggregates one run of whole records, and the thread
/// that finishes last merges the partial groups. Distinct names are split between the threads by
/// hash, so each name has one owner that counts it, and the threads hand names to their owners in
/// rounds.
///
/// Splitting a CSV in parallel is hard, because an LF inside a quoted field looks exactly like a
/// record end. The exact way is a pass that takes the parity of the quotes before each cut, but
/// that pass reads all 200 MB before any worker starts, and dropping it saved 4 to 5 ms (PERF.md,
/// step 10). Each run starts at the first LF after its cut that a digit or a BOM follows
/// instead, which takes a few bytes per cut. A record starts that way, and spec §2.2 rules it
/// out inside a note: a note's LF is followed by a lowercase word or a quote, and names hold no
/// LF. The first run starts right after the header line.
///
/// # The region table
///
/// Before the rounds, the same threads build the array from region id to group, in two steps
/// that each end at the barrier, and each thread takes one share of the rows (see
/// [`region_rows`]). In the first step it sets a bit in `seen` for each country code it reads.
/// Numbering the codes that occur by walking the bits in index order then sorts them bytewise
/// for free, so in the second step every thread numbers them and writes the group of each of its
/// rows. A pool of threads of its own for this, then the numbering and the array's writes on one
/// thread, kept the rounds from starting for 2.4 ms (PERF.md, step 33). Region ids ascend (spec
/// §2.3), so the last row sizes the array before the threads start. A byte holds the groups of
/// the spec's 240 countries and keeps the array at 1 MB at full scale.
///
/// # The exchange
///
/// The workers hand each name to the worker that owns it through mailboxes, `[half][from][to]`,
/// of [`MAILBOX`] letters of two words each (see [`name_letter`]), and `ends` says where the
/// letters in each end this round, as the index of the letter after them. There are two sets of
/// mailboxes, one per parity of the round. In round r a worker posts into set r % 2, which nobody
/// reads at that time: the set was last read in round r - 2, and every worker finished reading
/// it before it arrived at the barrier of round r - 1, which the poster has passed. So one
/// barrier per round is enough. The letters are atomics only so that safe code can share them;
/// every access is `Relaxed`, a plain load or store, and the barrier orders them.
///
/// Every worker takes part in every round, as many as the longest run needs, so the barrier
/// only counts arrivals: the barrier that ends step k is passed once `threads` × (k + 1) workers
/// have arrived, and round r is step r + 2. Zeroed memory this big comes as fresh pages from the
/// kernel, mapped in only when first written, so the mailbox pages that no round fills cost
/// nothing.
fn join(data: &[u8], regions: &[u8], threads: usize, out: &Path) -> ! {
    let len = data.len() - PAD;
    let body = 1 + data.iter().position(|&b| b == b'\n').expect("a header");
    // After an LF, a digit or a BOM, whose first byte is 0xEF.
    let start = |&k: &usize| data[k - 1] == b'\n' && matches!(data[k], b'0'..=b'9' | 0xEF);
    let run_start = |i| ((body + (len - body) * i / threads..len).find(start)).unwrap_or(len);
    let bounds: Vec<_> = (0..threads).map(run_start).chain([len]).collect();
    let rounds = (bounds.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0)).div_ceil(ROUND_BYTES);
    // SAFETY: `AtomicU64` has the same layout as `u64`, and all-zero bits are a valid 0.
    let zeroed = |len| unsafe { Box::<[AtomicU64]>::new_zeroed_slice(len).assume_init() };
    let letters = zeroed(2 * threads * threads * MAILBOX * 2);
    let (ends, arrived) = (zeroed(2 * threads * threads), AtomicU64::new(0));
    let mailbox = |half: usize, from: usize, to: usize| (half * threads + from) * threads + to;
    let mail = |mb: usize| &letters[mb * MAILBOX * 2..ends[mb].load(Relaxed) as usize * 2];
    let boxes = letters.as_chunks::<2>().0;
    // The region table (see above): the codes that occur, as bits, and the group of each id.
    let last_row = (regions[..regions.len() - PAD - 1].iter()).rposition(|&b| b == b'\n');
    // SAFETY: the LF lies before the pad, so the 8 bytes past it end inside the pad.
    let ids = 1 + last_row.map_or(0, |p| unsafe { number(regions, p + 1, b',') }.0 as usize);
    // SAFETY: `AtomicU8` has the same layout as `u8`, and all-zero bits are a valid 0.
    let by_id = unsafe { Box::<[AtomicU8]>::new_zeroed_slice(ids).assume_init() };
    // The array's pages are mapped in here, on one CPU, before the workers start. memcg charges
    // a CPU 64 pages at a time and memory.peak counts each batch whole, and when every worker
    // mapped in the pages of its own rows, the batches left partly used peaked 1 MiB higher
    // (PERF.md, step 33). Advice 23 is `MADV_POPULATE_WRITE`. A kernel without it fails the
    // call, and then the workers map the pages in.
    let (at, end) = (by_id.as_ptr().addr() & !4095, by_id.as_ptr().addr() + ids);
    // SAFETY: the advice changes no byte of memory. It maps in the pages of the range, all of
    // which hold the array or share its first page.
    unsafe { madvise(std::ptr::without_provenance(at), end - at, 23) };
    let seen = zeroed(CODES.div_ceil(64));
    let present = || (0..CODES).filter(|&c| seen[c / 64].load(Relaxed) >> (c % 64) & 1 == 1);
    let parts = std::sync::Mutex::new(Vec::new());
    // The hot loop, one run per worker `me`. The fields come in a fixed order, so the state is
    // simply the position in the record. [`number`] parses the id from one word, [`text_fields`]
    // skips the name and the note while counting their escapes, and [`region_amount`] parses the
    // region id and the amount from one vector. In each round the
    // worker parses about [`ROUND_BYTES`] of its records and posts each record's name to the
    // worker that owns it, waits at the barrier for all workers, and then marks the names posted
    // to it in its own [`NameSet`]. After the last round it gives its pages back, and the last
    // worker to finish writes the output.
    let work = |me: usize| {
        // Step 0 of the region table: the codes of this thread's share of the rows.
        let mut mine = [0u64; CODES.div_ceil(64)];
        region_rows(regions, me, threads, |_, c| mine[c / 64] |= 1 << (c % 64));
        (seen.iter().zip(mine)).for_each(|(s, m)| _ = s.fetch_or(m, Relaxed));
        barrier(&arrived, threads);
        // Step 1: the number of each code that occurs, and the group of each of this share's
        // ids. The numbers are freed before the groups are made, which then reuse their pages.
        let (mut num, mut group_count) = (vec![0; CODES], 1);
        for code in present() {
            num[code] = u8::try_from(group_count).expect("at most 255 countries");
            group_count += 1;
        }
        region_rows(regions, me, threads, |i, c| by_id[i].store(num[c], Relaxed));
        drop(num);
        barrier(&arrived, 2 * threads);
        // SAFETY: `AtomicU8` has the same layout as `u8`, and no thread writes the array after
        // the barrier, which orders every write before every read. Atomic loads would keep the
        // compiler from holding anything else in a register across them.
        let by_id = unsafe { &*(&raw const *by_id as *const [u8]) };
        let (mut at, end, mut groups) = (bounds[me], bounds[me + 1], vec![EMPTY; group_count]);
        // Full scale draws from 100,000 names (spec §2.2) in 202 MB (§5), about one per 2 KiB,
        // and a thread owns a share of them. The set grows when a file has more.
        let mut names = NameSet::new(len / 2048 / threads);
        let mut sent = vec![0; threads];
        // The batch of the record loop (see below). Zeroed at the start of every batch, it took
        // seven more instructions per four records. An entry holds its record's letter by
        // reference, so the group's store takes no second bounds check. The compiler lays out a
        // tuple as it likes, and in this order the id, the amount and the name's length, which
        // the merge puts in one vector, lie apart, so each is loaded from its own store. Side by
        // side, two of them were loaded as one, which cannot be forwarded from two stores, so it
        // waited for both to reach the cache, 0.44 to 0.64 M times per run (PERF.md, step 36).
        let mut batch = [(0, 0, 0, 0, 0, &boxes[0]); 4];
        for round in 0..rounds {
            let half = round % 2;
            (sent.iter_mut().enumerate()).for_each(|(to, s)| *s = mailbox(half, me, to) * MAILBOX);
            let round_end = end.min(at + ROUND_BYTES);
            while at < round_end {
                // The records of a batch are parsed and posted first, and added to their groups
                // after, so that the prefetch of each record's byte in the region table, which
                // misses the L2 cache about half the time, has the rest of the batch to arrive.
                let mut parsed = 0;
                while parsed < batch.len() && at < round_end {
                    // The input 448 bytes ahead, about seven records, so that its lines arrive
                    // before the parse needs them: the hardware prefetcher alone left 0.7 M more
                    // L1 misses per run on the input's loads (PERF.md, step 34).
                    let ahead = data.as_ptr().wrapping_add(at + 448);
                    // SAFETY: a prefetch reads nothing the program sees, and never faults.
                    unsafe { _mm_prefetch::<_MM_HINT_T0>(ahead.cast()) };
                    // A BOM at the start of a record is not data (§2.1). It sits in front of the
                    // id, so missing it would show up in `id_sum`. A record starts with a digit
                    // or a BOM, so a first byte of 0xEF is a BOM. The id's word already holds
                    // that byte, so the test takes no load, and only after a BOM is the id parsed
                    // twice.
                    // SAFETY: `at` is below `end`, at most the input's length, so the 8 bytes from
                    // it, and from the 3 past it after a BOM, end inside the pad.
                    let (mut id, mut comma) = unsafe { number(data, at, b',') };
                    // SAFETY: as above. Checked, the byte took two more instructions.
                    if unsafe { *data.get_unchecked(at) } == BOM[0] {
                        // SAFETY: as above.
                        (id, comma) = unsafe { number(data, at + BOM.len(), b',') };
                    }
                    let (name, name_len, note_len, after_note) = text_fields(data, comma + 1);
                    // SAFETY: the name ends at the latest at the comma after it, which
                    // `text_fields` found. The 64 bytes of the pad are zeros, so the comma lies
                    // before them, and the name ends more than 64 bytes before the end of `data`.
                    let letter = unsafe { name_letter(data, name) };
                    // SAFETY: `after_note` is one past a comma of the input. The 64 bytes of the
                    // pad are zeros, so the comma lies before them, and the 16 bytes from it on
                    // lie in `data`.
                    let (region_id, cents, dot) = unsafe { region_amount(data, after_note) };
                    let byte = by_id.as_ptr().wrapping_add(region_id as usize);
                    // SAFETY: a prefetch reads nothing the program sees, and never faults.
                    unsafe { _mm_prefetch::<_MM_HINT_T0>(byte.cast()) };
                    // Past the CR if any, then the LF. Only the last record may end at end of
                    // file instead (§2.1), which leaves `at` past the end and stops the loop. CR
                    // is 0x0D and LF 0x0A, so the byte's lowest bit tells them apart, with one
                    // instruction fewer than a compare with CR.
                    // SAFETY: `region_amount` puts the dot at most 13 bytes past `after_note`,
                    // and `region_amount_slow` at most at the pad's second byte. `after_note` is
                    // at most the input's length (see above), so the byte 3 past the dot lies in
                    // the pad at the latest. Checked, the read took three more instructions.
                    at = dot + 4 + usize::from(unsafe { *data.get_unchecked(dot + 3) } & 1);
                    // The thread count as `sent.len()`: `threads`, read through the closure's
                    // reference to it, took one more load per record.
                    let to = ((u64::from(letter[1] as u32) * sent.len() as u64) >> 32) as usize;
                    // Read once, because the compiler loaded `sent[to]` again for each of the four
                    // atomic stores that posted a letter (PERF.md, step 34), and unchecked, which
                    // saves its bounds check.
                    // SAFETY: the hash is below 2^32, so `to` is below `sent.len()`.
                    let sent = unsafe { sent.get_unchecked_mut(to) };
                    let slot = *sent;
                    // One 16-byte store. The compiler does not merge atomic stores, and the two
                    // that posted the letter took 7 more instructions per record (PERF.md, step
                    // 41).
                    // SAFETY: the letter's atomics are writable through a shared reference, and
                    // no other thread reads or writes them before the barrier.
                    unsafe { std::ptr::write(boxes[slot].as_ptr().cast_mut().cast(), letter) };
                    batch[parsed] = (id, region_id, cents, note_len, name_len, &boxes[slot]);
                    (*sent, parsed) = (slot + 1, parsed + 1);
                }
                for &(id, region_id, cents, note_len, name_len, top) in &batch[..parsed] {
                    // Records also reference ids past the table's largest (§2.2), so the lookup
                    // is bounds-checked; those ids are unmatched like the holes inside the table.
                    let group = by_id.get(region_id as usize).map_or(0, |&g| g as usize);
                    // The group goes into byte 15 of the letter, the top byte of its hash, which
                    // no owner reads as hash (see [`NameSet`]). Storing the byte alone takes one
                    // instruction and no load, where an OR into the word took three.
                    let byte = std::ptr::from_ref(top).cast_mut().cast::<u8>();
                    // SAFETY: the byte is inside the letter's atomics, writable through a shared
                    // reference, and no other thread reads or writes them before the barrier.
                    unsafe { byte.add(15).write(group as u8) };
                    let record = [1, id as i64, cents, name_len, note_len, 0, cents, cents];
                    // SAFETY: `by_id` holds 0 or a number from step 1, below `group_count`, the
                    // length of `groups`. The bounds check took two instructions per record.
                    merge(unsafe { groups.get_unchecked_mut(group) }, &record);
                }
            }
            for (to, &next) in sent.iter().enumerate() {
                ends[mailbox(half, me, to)].store(next as u64, Relaxed);
            }
            barrier(&arrived, (round + 3) * threads);
            (0..threads).for_each(|from| names.read(mail(mailbox(half, from, me)), &mut groups));
        }
        // The run's pages go back to the kernel with `madvise(MADV_DONTNEED)`, and then the name
        // set's entries. At exit the kernel unmaps every page left on one thread, inside the timed
        // window: the input's took 2.8 ms (an explicit `munmap`), and the entries 0.4 ms when they
        // were 6.4 MB (PERF.md, step 38), so each worker drops its own here, in parallel. Every
        // worker is done parsing by the last barrier, and with its name set after its last round;
        // dropping pages earlier would slow the threads still parsing, because it makes the kernel
        // flush every thread's TLB. The set is at its largest here, so memory.peak has counted its
        // pages.
        let page = bounds[me] & !4095;
        // SAFETY: `data` is the read-only file mapping from [`map_file`], which starts a page,
        // and the range, rounded out to whole pages, stays inside the file's pages. A dropped
        // page that is read again, such as the end of the run before, maps the same bytes from
        // the file, so the call is only advice and its result is ignored. Advice 4 is
        // `MADV_DONTNEED`.
        unsafe { madvise(data[page..].as_ptr(), bounds[me + 1] - page, 4) };
        let (at, len) = (names.entries.as_ptr().addr(), size_of_val(&*names.entries));
        let start = std::ptr::without_provenance(at.next_multiple_of(4096));
        // SAFETY: the range is the whole pages inside the entries, which hold no other buffer's
        // bytes, and no entry is read again. The set itself is never freed: its `munmap` calls,
        // one worker at a time under the address space's lock, made the runs 0.7 ms slower.
        unsafe { madvise(start, (at + len).saturating_sub(start.addr()) & !4095, 4) };
        // The last worker to get here merges the groups, writes the output and ends the process,
        // which ends the other workers too. Workers that returned instead would end their
        // threads first, in calls that took 0.8 ms for all of them (PERF.md, step 38). Nothing is
        // freed, since the exit unmaps it all, and `fs::write` keeps no user-space buffer.
        // A panic aborts the process, so the lock is never poisoned.
        let mut all = parts.lock().unwrap();
        if all.len() + 1 < threads {
            all.push(groups);
            drop(all);
            // Asleep until the end. A spurious wakeup would only end this thread early.
            return thread::park();
        }
        let code = |c: usize| [c / 676, c / 26 % 26, c % 26].map(|c| b'A' + c as u8);
        let codes: Vec<_> = present().map(code).collect();
        // Every part numbers its groups as `groups` does, so the parts merge into it pairwise.
        (all.iter()).for_each(|part| (groups.iter_mut().zip(part)).for_each(|(g, p)| merge(g, p)));
        write(out, format_output(&groups, &codes)).expect("a writable output directory");
        std::process::exit(0)
    };
    // One thread per worker. The scope never ends: the last worker to finish ends the process,
    // which also ends the others.
    thread::scope(|s| (0..threads).for_each(|me| _ = s.spawn(move || work(me))));
    unreachable!("the last worker ends the process")
}

/// Arrives at the barrier that `arrived` counts, and waits until `all` workers have arrived.
///
/// The `Release` add passes this worker's writes on, and a waiter's `Acquire` load that sees
/// every worker's add sees every write made before it. Workers usually arrive within
/// microseconds of each other, so a waiter spins first. A worker that is late because the
/// scheduler put it aside would keep every spinner busy for whole time slices, and busy CPUs
/// never look idle enough for the scheduler to move it. So a waiter that has spun long sleeps in
/// short steps, which frees its CPU.
fn barrier(arrived: &AtomicU64, all: usize) {
    arrived.fetch_add(1, Ordering::Release);
    for spins in (0..).take_while(|_| (arrived.load(Ordering::Acquire) as usize) < all) {
        std::hint::spin_loop();
        (spins > SPINS).then(|| thread::sleep(Duration::from_micros(50)));
    }
}

/// Adds `other`'s records to `group`. The hot loop in [`join`] also adds each single record this
/// way, as a group of one, so there is one place that knows how fields combine.
fn merge(group: &mut Group, other: &Group) {
    (0..6).for_each(|i| group[i] += other[i]);
    [group[6], group[7]] = [group[6].min(other[6]), group[7].max(other[7])];
}

/// Parses the decimal digits from `at` up to the `stop` byte, and returns the value and the
/// position of `stop`. The field's end and its value come out of the same pass.
///
/// A field that fits in 8 bytes with its `stop` is parsed from one 8-byte word, with no loop and
/// no branch on its length (SWAR: SIMD within a register). Subtracting `0` from every byte leaves
/// each digit's value in its byte and sets the top bit of each byte below `0`, which every `stop`
/// is (`,` or `.`). Shifting the digits to the top of the word turns the missing ones into leading
/// zeros and drops `stop` and the bytes after it, and three multiplies then join neighbouring
/// digits into pairs, pairs into fours and fours into the value. Longer fields, such as an id with
/// leading zeros (§2.1 allows them), take the loop over bytes.
///
/// It is always inlined, so the id's parse at the start of each record is not a call.
///
/// # Safety
///
/// The 8 bytes from `at` on lie in `data`. Every caller starts it at most a few bytes past the
/// input's end, which the [`PAD`] follows, so the read is not checked: its check took about
/// five instructions per record.
#[inline(always)]
unsafe fn number(data: &[u8], mut at: usize, stop: u8) -> (u64, usize) {
    const ONES: u64 = u64::from_le_bytes([1; 8]);
    // SAFETY: the caller keeps the 8 bytes in `data`, and `[u8; 8]` needs no alignment.
    let word = u64::from_le_bytes(unsafe { *data.as_ptr().add(at).cast() });
    let digits = word.wrapping_sub(ONES * u64::from(b'0'));
    // The lowest set bit marks the first byte below `0`, so the first `stop`. Bits above it
    // can be false, which is why only the lowest one is used.
    let stops = digits & !word & (ONES << 7);
    if stops != 0 {
        let len = stops.trailing_zeros() as usize / 8;
        // The subtraction borrows at `stop` and spoils the bytes after it, which the shift drops.
        // In two steps, because a 0-digit field would need a shift by 64.
        let digits = digits << (8 * (7 - len)) << 8;
        let pairs = (digits.wrapping_mul(10 << 8 | 1) >> 8) & 0x00FF_00FF_00FF_00FF;
        let fours = (pairs.wrapping_mul(100 << 16 | 1) >> 16) & 0x0000_FFFF_0000_FFFF;
        return (fours.wrapping_mul(10000 << 32 | 1) >> 32, at + len);
    }
    let mut value = 0;
    while data[at] != stop {
        (value, at) = (value * 10 + u64::from(data[at] - b'0'), at + 1);
    }
    (value, at)
}

/// Parses the region id and the amount at `at`, `region_id,amount`, and returns the region id,
/// the amount in cents and the position of the amount's dot.
///
/// Spec §2.2 bounds region ids at 9,999,999 and amounts at -999.99 to 99,999.99, so both fields,
/// with their comma, the dot and the cents, fit in 16 bytes, and one SSE register parses them
/// together. The steps are those of [`number`], in a vector:
///
/// 1. A compare with `/` marks the digits. The first byte that is not one is the comma, and the
///    first past the byte after the comma, which is a digit or the sign, is the dot. Bit 16 stands
///    for the byte past the window, so a field that runs out of the window fails the range check.
/// 2. Subtracting `0` with saturation leaves each digit's value in its byte and turns every other
///    byte, the sign included, into 0.
/// 3. One `pshufb`, with the control for that comma and dot (see [`ALIGN`]), moves the region id's
///    digits to the top of the low 8 bytes and the amount's to the top of the high 8, without the
///    dot, so the missing digits become leading zeros.
/// 4. Multiply-adds join neighbouring digits into pairs, pairs into fours and fours into the two
///    values, which come out as the halves of one 64-bit word.
///
/// Two parses of one word each took dependent shifts, multiplies and branches for each field;
/// this runs 34 fewer instructions per record (PERF.md, step 35). Fields that §2.1 allows past
/// those bounds, such as a region id with leading zeros, take [`region_amount_slow`].
///
/// # Safety
///
/// `at` is at most the input's length, `data.len() - PAD`, so the 16 bytes from it on lie in
/// `data`, and so do the reads of [`region_amount_slow`]. The record loop knows it from the
/// comma before `at`, so the read is not checked again: its check took five instructions per
/// record.
unsafe fn region_amount(data: &[u8], at: usize) -> (u64, i64, usize) {
    // SAFETY: the build targets Alder Lake, which has SSE2 (see `.cargo/config.toml`), and the
    // caller keeps the 16 bytes from `at` in `data`. `loadu` needs no alignment.
    let window = unsafe { _mm_loadu_si128(data.as_ptr().add(at).cast()) };
    // SAFETY: as above.
    let digits = unsafe { _mm_movemask_epi8(_mm_cmpgt_epi8(window, _mm_set1_epi8(b'/' as i8))) };
    let stops = digits.cast_unsigned() ^ 0x1_FFFF;
    let comma = stops.trailing_zeros() as usize;
    let dot = (stops & !(3 << comma)).trailing_zeros() as usize;
    // An empty region id, which §2.2 rules out, passes and parses as 0, as it does in the slow
    // path, so the comma's test is one compare.
    if comma >= 8 || dot.wrapping_sub(comma + 2) >= 5 {
        // SAFETY: the caller keeps `at` at most the input's length.
        return unsafe { region_amount_slow(data, at) };
    }
    let control = ALIGN[(comma << 4 | dot) & 127];
    // SAFETY: the build targets Alder Lake, which has SSSE3 and SSE4.1 (see `.cargo/config.toml`).
    let both = unsafe {
        let aligned = _mm_shuffle_epi8(_mm_subs_epu8(window, _mm_set1_epi8(b'0' as i8)), control);
        // The weights of each lane's halves, low half first: 10 and 1, 100 and 1, 10000 and 1.
        let pairs = _mm_maddubs_epi16(aligned, _mm_set1_epi16((1 << 8) + 10));
        let fours = _mm_madd_epi16(pairs, _mm_set1_epi32((1 << 16) + 100));
        let fours = _mm_packus_epi32(fours, fours);
        _mm_cvtsi128_si64(_mm_madd_epi16(fours, _mm_set1_epi32((1 << 16) + 10000))).cast_unsigned()
    };
    let negative = stops >> (comma + 1) & 1 == 1;
    let cents = (both >> 32).cast_signed() * if negative { -1 } else { 1 };
    (both & 0xFFFF_FFFF, cents, at + dot)
}

/// [`region_amount`] for fields that do not fit its window. No record of a generated file takes
/// it (§2.2), so it is cold and out of line.
///
/// # Safety
///
/// `at` is at most the input's length, `data.len() - PAD`, as for [`region_amount`].
#[cold]
unsafe fn region_amount_slow(data: &[u8], at: usize) -> (u64, i64, usize) {
    // SAFETY: the caller keeps `at` at most the input's length, so the 8 bytes from it end
    // inside the pad.
    let (region_id, comma) = unsafe { number(data, at, b',') };
    // The sign applies to the whole value: -0.75 is -75 cents, not -0 * 100 + 75.
    let negative = data[comma + 1] == b'-';
    // SAFETY: the comma is the first byte below `0` in the word from `at`, so at most the pad's
    // first byte, or a real comma that the loop found. The word starts one byte past it, or two
    // past it after a real `-`, so at most at the pad's second byte.
    let (units, dot) = unsafe { number(data, comma + 1 + usize::from(negative), b'.') };
    let cents = (1..=2).fold(units, |c, i| c * 10 + u64::from(data[dot + i] - b'0')) as i64;
    (region_id, if negative { -cents } else { cents }, dot)
}

/// The `pshufb` controls of [`region_amount`], one for each place of the comma and the dot, at
/// `comma << 4 | dot`. Byte j of a control names the window byte that goes to byte j, and 0x80
/// puts a zero there. Byte j < 8 takes byte j + comma - 8, which right-aligns the region id in
/// the low half. Bytes 8 to 13 take j + dot - 14, the amount's sign and integer digits, and bytes
/// 14 and 15 take j + dot - 13, its cents, which skips the dot. A byte whose source lies before
/// its field is 0x80. SSE shifts bytes only by constants, so each layout needs a control of
/// its own, and as vectors the controls are 16-byte aligned, so none straddles two cache lines.
static ALIGN: [__m128i; 128] = {
    let (mut table, mut i) = ([[0; 16]; 128], 0);
    while i < 128 * 16 {
        let (comma, dot, j) = (i >> 8, i >> 4 & 15, i & 15);
        // The source plus 14, which keeps it from going below 0, and its field's start plus 14.
        let [src, start] = [[j + comma + 6, 14], [j + dot + j / 14, comma + 15]][j / 8];
        table[i >> 4][j] = if src >= start { (src - 14) as u8 } else { 0x80 };
        i += 1;
    }
    // SAFETY: `__m128i` is 16 bytes, and every bit pattern is a valid one.
    unsafe { std::mem::transmute(table) }
};

/// Reads the name and the note at `at`. Returns the name's raw bytes (between its quotes, still
/// escaped), the lengths of both after unescaping, and the position after the note's comma.
///
/// Unescaping would copy every field. The spec's byte counts need only the lengths: a quoted
/// field holds its value, the two quotes around it and one more quote per quote in it, so the
/// value is the field's length less half its quotes, less one. Distinct counting needs no
/// unescaping either (see [`NameSet`]).
///
/// The 64 bytes from the name's start hold the name and the note of 98% of the records at full
/// scale, so both fields come from two masks of those bytes, of the quotes and of the commas,
/// with no branch per field or per quote. Inside quotes a comma is data, and a byte is inside
/// them when an odd number of quotes come before it (see [`prefix_xor`]), so the first two commas
/// outside end the name and the note. An escaped `""` flips the parity twice, so a note can hold
/// `"",`, which a test for a quote and then a comma would take for its end, or open or close with
/// `"""`. Unquoted text has no quotes (§2.1). A note that ends past the window takes
/// [`text_fields_slow`].
///
/// Walking each field's quotes cost a branch per quote, which records with an escaped quote
/// mispredicted: this cut branch misses from 2.9 to 1.9 M per run (PERF.md, step 35).
fn text_fields(data: &[u8], at: usize) -> (Range<usize>, i64, i64, usize) {
    let (quotes, commas) = (byte_mask(data, at, b'"'), byte_mask(data, at, b','));
    let ends = commas & !prefix_xor(quotes);
    let (first, rest) = (ends.trailing_zeros() as usize, ends & ends.wrapping_sub(1));
    if rest == 0 {
        return text_fields_slow(data, at);
    }
    let second = rest.trailing_zeros() as usize;
    // `bzhi` keeps the bits below `i`. As `(1 << i) - 1` the count was first masked to 63, since
    // the compiler does not know that `first` is below 64: two more instructions per record.
    // SAFETY: the build targets Alder Lake, which has BMI2 (see `.cargo/config.toml`).
    let before = |i: usize| unsafe { _bzhi_u64(quotes, i as u32) }.count_ones() as usize;
    let (q1, q2) = ((quotes & 1) as usize, (quotes >> (first + 1) & 1) as usize);
    let (in_name, in_both) = (before(first), before(second));
    let name_len = (first - in_name / 2 - q1) as i64;
    let note_len = (second - first - 1 - (in_both - in_name) / 2 - q2) as i64;
    let name = at + q1..at + first - q1;
    (name, name_len, note_len, at + second + 1)
}

/// [`text_fields`] for a record whose note ends past the window: the same masks, 64 bytes at a
/// time, with the parity of the quotes carried from each block to the next. About 2% of the
/// records come here, so it is cold and out of line.
#[cold]
fn text_fields_slow(data: &[u8], at: usize) -> (Range<usize>, i64, i64, usize) {
    // The commas that end the name and the note, each with the number of quotes before it.
    let (mut ends, mut found, mut block, mut odd, mut seen) = ([[0; 2]; 2], 0, at, 0, 0);
    while found < 2 {
        let quotes = byte_mask(data, block, b'"');
        let inside = prefix_xor(quotes) ^ odd;
        let mut commas = byte_mask(data, block, b',') & !inside;
        while commas != 0 && found < 2 {
            let i = commas.trailing_zeros() as usize;
            let before = seen + (quotes & ((1 << i) - 1)).count_ones() as usize;
            (ends[found], found, commas) = ([block + i, before], found + 1, commas & (commas - 1));
        }
        // All ones when the block ends inside quotes.
        odd = 0u64.wrapping_sub(inside >> 63);
        (block, seen) = (block + 64, seen + quotes.count_ones() as usize);
    }
    let [[name, in_name], [note, in_both]] = ends;
    // A field is quoted when it holds a quote (§2.1).
    let (q1, q2) = (usize::from(in_name > 0), usize::from(in_both > in_name));
    let name_len = (name - at - in_name / 2 - q1) as i64;
    let note_len = (note - name - 1 - (in_both - in_name) / 2 - q2) as i64;
    (at + q1..name - q1, name_len, note_len, note + 1)
}

/// The prefix XOR of `bits`: bit i is the parity of bits 0 to i. Of a mask of quotes, it marks
/// the bytes inside quotes, each opening quote with them. A carry-less multiply by all ones
/// computes it in one instruction, as simdjson does.
fn prefix_xor(bits: u64) -> u64 {
    // SAFETY: the build targets Alder Lake, which has PCLMULQDQ and SSE2 (see
    // `.cargo/config.toml`).
    unsafe {
        let ones = _mm_set1_epi8(-1);
        let odd = _mm_clmulepi64_si128::<0>(_mm_cvtsi64_si128(bits.cast_signed()), ones);
        _mm_cvtsi128_si64(odd).cast_unsigned()
    }
}

/// The bytes equal to `byte` among the 64 at `at`, as a mask with bit i set where `data[at + i]`
/// is `byte`: two AVX2 compares of 32 bytes, each packed into 32 bits by `vpmovmskb`. Written as
/// a loop over bytes, the compare that starts each record's window became byte loads and shifts.
fn byte_mask(data: &[u8], at: usize, byte: u8) -> u64 {
    let bytes: &[u8; 64] = data[at..].first_chunk().expect("the input is padded");
    // SAFETY: the build targets Alder Lake, which has AVX2 (see `.cargo/config.toml`), and both
    // 32-byte loads lie in `bytes`. `loadu` needs no alignment.
    let half = |i: usize| unsafe {
        let v = _mm256_loadu_si256(bytes[i..].as_ptr().cast());
        _mm256_movemask_epi8(_mm256_cmpeq_epi8(v, _mm256_set1_epi8(byte.cast_signed())))
    };
    u64::from(half(0).cast_unsigned()) | u64::from(half(32).cast_unsigned()) << 32
}

/// A letter for the name at `start..end` of `data`: a 128-bit hash of the name, as two words,
/// whose top byte the poster overwrites with the record's group. The owner knows the name by its
/// hash alone (see [`NameSet`]). A letter used to carry the name's first 24 bytes, a CRC32 and
/// the length, 32 bytes, and a longer name as its offsets in the file, where the owner compared
/// it. A 16-byte letter halves the exchange's traffic, and the owner's entry for a name shrinks
/// from 64 bytes to 40 (PERF.md, step 41).
///
/// A name of up to 32 bytes comes from one fixed-size read past its start, masked to its length,
/// as two 16-byte blocks, `a` and `b`. The bytes after a name are the rest of its record, and
/// the [`PAD`] follows the input, so the read always fits. Names hold no NUL byte (§2.2), so the
/// blocks are one to one with the name, trailing spaces included. Three AES rounds, one
/// instruction each, chain them: the first takes `a` with `b` as its round key, which is added
/// after it, and two more with [`KEY`] give the hash, `aesenc(aesenc(aesenc(a, b), KEY), KEY)`.
/// So `a` passes three rounds and `b` two. A round is a permutation of its input, so two names
/// with the same `a` get different hashes for certain, and so do two with the same `b`. Two
/// names one byte apart, such as `Betts` and `Betts `, differ in all four bytes of a column
/// after the first round that takes the byte through its S-box, since `MixColumns` spreads one
/// byte over its column, then in all 16 bytes after the next round, and so in every column of
/// the hash, including the two of its low word, the key. Other names agree in a given 72 bits of
/// their hashes with odds of about 2^-72, as for a random function. With `b` as a round key, the
/// hash takes three instructions, where `aesenc(aesenc(aesenc(a ^ K0, K1) ^ b, K2), K3)`, which
/// passes `a` and `b` through as many rounds, took five (PERF.md, step 41). A name over 32 bytes
/// takes [`long_letter`].
///
/// # Safety
///
/// The 32 bytes from the start of the name lie in `data`, and if the name is longer than that,
/// so do the 32 bytes after its end. The record loop knows it from the comma that ends the name,
/// a byte of the input, so the read is not checked again: its check took four instructions per
/// record (step 39).
unsafe fn name_letter(data: &[u8], Range { start, end }: Range<usize>) -> [u64; 2] {
    let len = end - start;
    if len > 32 {
        // SAFETY: the caller keeps the contract of `long_letter`, which is this one.
        return unsafe { long_letter(data, start, end) };
    }
    // SAFETY: the caller keeps the 32 bytes from `start` in `data`, and the 32 bytes of `KEEP`
    // from `32 - len` on lie in it. The build targets Alder Lake, which has AVX2 and AES-NI (see
    // `.cargo/config.toml`), and `loadu` needs no alignment. `__m256i` is 32 bytes, as the two
    // blocks are, and `__m128i` 16, as the hash is, and every bit pattern of each is valid.
    unsafe {
        let name = _mm256_loadu_si256(data.as_ptr().add(start).cast());
        let keep = _mm256_loadu_si256(KEEP.as_flattened().as_ptr().add(32 - len).cast());
        let [a, b]: [__m128i; 2] = std::mem::transmute(_mm256_and_si256(name, keep));
        let x = _mm_aesenc_si128(_mm_aesenc_si128(a, b), KEY);
        std::mem::transmute(_mm_aesenc_si128(x, KEY))
    }
}

/// [`name_letter`] for a name over 32 bytes. The first 32 bytes and the rest each get a hash,
/// and the two hashes, with the length in the first, are hashed as the 32 bytes of a name, so
/// that every byte and the length enter the hash. No generated name is that long (the longest of
/// full seed 1 has 27 bytes), so it is cold and out of line.
///
/// # Safety
///
/// As for [`name_letter`] with a name over 32 bytes: the 32 bytes after its end lie in `data`.
#[cold]
unsafe fn long_letter(data: &[u8], start: usize, end: usize) -> [u64; 2] {
    // SAFETY: the first part lies in the name. The rest ends where the name does, and its first
    // 32 bytes end before the 32 after the name do, so the caller's contract holds for both.
    let [[a, b], c] = [start..start + 32, start + 32..end].map(|r| unsafe { name_letter(data, r) });
    let words = [a ^ (end - start) as u64, b, c[0], c[1]].map(u64::to_le_bytes);
    // SAFETY: the name is all 32 bytes of `words`.
    unsafe { name_letter(words.as_flattened(), 0..32) }
}

/// One [`NameSet`] entry, 40 bytes: the name's key, 64 bits of its hash, then one bit per group.
/// 255 countries and `_UNMATCHED` fill the 256 bits. A key and the word of its bit mostly share a
/// cache line: as two dense arrays, keys and bits, the marking took 21% more L1 misses and the
/// runs 0.6% longer (PERF.md, step 41).
struct Entry(u64, [u64; 4]);

/// One worker's share of the distinct names, with the countries each name occurs in.
///
/// It is a hash table with open addressing and linear probing that only its worker ever
/// touches: no atomics and no cache line shared with another core. A slot holds an entry's
/// number and 8 bits of the name's hash (see [`ENTRY`]), so an empty slot costs 4 bytes, and a
/// probe loads an entry only when those bits match. An entry holds 64 more bits of the hash, the
/// key, and the name's group bits, 40 bytes, where the cache line that held the name's bytes
/// took 64 (PERF.md, step 41).
///
/// A name is known by its hash alone (see [`name_letter`]). The letter's low word is the key, whose
/// low bits give the slot, byte 14 is the tag, and bytes 8 to 11 picked the owner, so each comes
/// from other bits. Two names are taken for one only if their tags and keys agree, 72 bits. At
/// full scale each of 12 workers owns about 8,400 of the 100,000 names, 35 M pairs of names, so
/// a run merges two names with odds of about 12 × 35 M / 2^72, 1 in 10^13. A single worker would
/// own them all, 5 × 10^9 pairs, 1 in 10^12.
///
/// Names are hashed still escaped. Escaping is one to one (each `"` becomes `""`), so two raw
/// names are equal exactly when their unescaped values are. The empty quoted name `""` and the
/// empty unquoted name both arrive as an empty slice, so they are one name, as spec §3 asks.
struct NameSet {
    /// The entry of each name, with 8 bits of its hash below it (see [`ENTRY`]), or 0.
    slots: Vec<u32>,
    entries: Vec<Entry>,
}

impl NameSet {
    /// An empty set with room for about `names` names.
    fn new(names: usize) -> Self {
        // The slots may fill to three quarters before they grow (see `insert`). Pages of the
        // entries' capacity that no entry reaches cost nothing, so it is generous.
        let slots = vec![0; (4 * names / 3).next_power_of_two().max(16)];
        let entries = Vec::with_capacity(2 * names);
        Self { slots, entries }
    }

    /// Marks the names of `letters` and counts each name that is new for its group in
    /// `groups`. Kept out of line so that this loop gets the registers to itself.
    #[inline(never)]
    fn read(&mut self, letters: &[AtomicU64], groups: &mut [Group]) {
        for [key, high] in letters.as_chunks().0 {
            let (key, high) = (key.load(Relaxed), high.load(Relaxed));
            // A group fits in a byte (see [`join`]), so the bit's word is one of four with no
            // bounds check.
            let (mask, tag) = (self.slots.len() - 1, (high >> 48) as u8);
            let (group, mut slot) = ((high >> 56) as usize, key as usize & mask);
            let entry = loop {
                let found = self.slots[slot];
                match (found >> 8) as usize {
                    0 => break self.insert(slot, tag, key),
                    e if found as u8 == tag && self.entries[e - 1].0 == key => {
                        break &mut self.entries[e - 1];
                    }
                    _ => slot = (slot + 1) & mask,
                }
            };
            let (word, bit) = (&mut entry.1[group / 64], 1 << (group % 64));
            // SAFETY: the group is the byte that the letter's poster took from `by_id`, 0 or a
            // number below `group_count`. Every worker counts the same codes, so that is the
            // length of `groups` here too (see [`join`]).
            unsafe { groups.get_unchecked_mut(group)[5] += i64::from(*word & bit == 0) }
            *word |= bit;
        }
    }

    /// Appends an entry with the key and no groups, puts it in the empty `slot` with its `tag`,
    /// and returns it. When the entries fill three quarters of the slots, the slots double and
    /// every slot is placed again by its entry's key, so every probe ends at an empty slot. The
    /// slots are placed from the old ones, which hold the tags that no entry keeps.
    ///
    /// Cold, because inlined into [`NameSet::read`] it spilled the key of every letter.
    #[cold]
    fn insert(&mut self, slot: usize, tag: u8, key: u64) -> &mut Entry {
        assert!(self.entries.len() < ENTRY as usize, "over 2^24 names");
        self.entries.push(Entry(key, [0; 4]));
        self.slots[slot] = (self.entries.len() as u32) << 8 | u32::from(tag);
        if 4 * self.entries.len() > 3 * self.slots.len() {
            let mask = 2 * self.slots.len() - 1;
            let old = std::mem::replace(&mut self.slots, vec![0; mask + 1]);
            for found in old.into_iter().filter(|&found| found != 0) {
                let mut slot = self.entries[(found >> 8) as usize - 1].0 as usize & mask;
                while self.slots[slot] != 0 {
                    slot = (slot + 1) & mask;
                }
                self.slots[slot] = found;
            }
        }
        self.entries.last_mut().expect("an entry was just pushed")
    }
}

/// Writes one line per group that has records: the countries in bytewise order, then
/// `_UNMATCHED`, which sorts after every capital letter because `_` is 0x5F (spec §4).
///
/// Group numbers already follow bytewise order (see [`join`]), so this is a walk, not
/// a sort. Money prints as `-?I.DD`: -525 is `-5.25`, -5 is `-0.05`, and zero is `0.00`, never
/// `-0.00`, because the sign is printed only for a negative value.
fn format_output(groups: &[Group], codes: &[[u8; 3]]) -> Vec<u8> {
    let money = |c: i64| {
        let (sign, abs) = (if c < 0 { "-" } else { "" }, c.abs());
        format!("{sign}{}.{:02}", abs / 100, abs % 100)
    };
    let mut out = Vec::new();
    // Group k + 1 is country k, and group 0, `_UNMATCHED`, goes last.
    let keys = (codes.iter().map(|code| &code[..])).chain([&b"_UNMATCHED"[..]]);
    let lines = keys.zip(groups[1..].iter().chain(&groups[..1]));
    // A country without records prints nothing (§3).
    for (key, group) in lines.filter(|(_, group)| group[0] > 0) {
        let [count, ids, sum, name_bytes, note_bytes, distinct, min, max] = *group;
        let (sum, min, max) = (money(sum), money(min), money(max));
        out.extend_from_slice(key);
        write!(out, " {count} {ids} {sum} {min} {max}").unwrap();
        writeln!(out, " {distinct} {name_bytes} {note_bytes}").unwrap();
    }
    out
}
