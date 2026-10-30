//! # Track B: Index
//!
//! This program answers 50,000 queries over a corpus of about 250,000 documents, one per line
//! of `docs.txt`, and writes one line per query: the number of matching documents, the sum of
//! their ids and the ten smallest ids (spec §4). A query is a boolean expression evaluated
//! strictly left to right, or an exact phrase:
//!
//! ```text
//! BOOL estate OR surety AND bail
//! PHRASE "writ of habeas corpus"
//! ```
//!
//! ## The pipeline
//!
//! 1. **Queries first.** We read `queries.txt` before the corpus (spec §3 allows it) and give
//!    every distinct query term a small id ([`TermTable`]). A corpus token that no query uses
//!    is looked up once and then forgotten.
//! 2. **Triggers.** Tokenizing the first [`SAMPLE_DOCS`] documents estimates how many
//!    documents contain each term ([`sample_hits`]). Every query then gets *triggers*, terms
//!    such that each matching document contains at least one of them ([`Plan`]): a phrase's
//!    rarest term, or a BOOL query's [`bool_cover`]. Triggers decide how much work is done,
//!    never the answer. A BOOL query whose cover holds a common term would be checked on too
//!    many documents, so it is evaluated on whole sets instead.
//! 3. **One pass on every thread.** Each thread scans a chunk of whole lines ([`scan`]), and
//!    [`walk`] finds the tokens 64 bytes at a time from AVX2 bit masks. At the end of each
//!    document, every query its triggers point at is checked against that document alone: a
//!    phrase window by window, a BOOL query by applying its operators to "does this document
//!    contain the term" ([`run_checks`]). Set operations act on each document separately,
//!    so this gives the same answer as combining whole sets (spec §3). For the queries on
//!    sets, the pass records which documents hold their terms: a bitset per common term and
//!    an id list per rare one ([`Chunk`]).
//! 4. **Merge.** Per query, the chunks' counts and id sums add up, and the smallest ids come
//!    from the earliest chunks ([`Tally`]). The queries on sets combine bitsets 64 documents
//!    per operation ([`on_sets`]).
//!
//! Memory stays at a few arrays per thread plus the sets: one bit per document for each
//! common term and 32 bits per document for each rare one. Numbers: PERF.md.

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::thread;

/// Documents tokenized up front to estimate how rare each query term is.
const SAMPLE_DOCS: usize = 1000;
/// The term id of a corpus token that no query uses.
const MISS: u32 = u32::MAX;
/// An output line lists at most this many ids (spec §4).
const SHOWN_IDS: usize = 10;
/// A term is common when at least one sampled document in this many holds it: its bitset
/// is then no bigger than a list of 32-bit ids.
const DENSE_SHARE: usize = 32;

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
}
const PROT_READ: i32 = 1;
const MAP_PRIVATE: i32 = 2;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let data_dir = args.next().unwrap_or_else(|| "/data".into());
    let out_dir = args.next().unwrap_or_else(|| "/out".into());
    let data = Path::new(&data_dir);
    let queries = map_file(&data.join("queries.txt"))?;
    let docs = map_file(&data.join("docs.txt"))?;
    let parts = answer_all(queries, docs)?;
    let out_path = Path::new(&out_dir).join("results.txt");
    let mut out = File::create(&out_path).map_err(|e| format!("{}: {e}", out_path.display()))?;
    for part in &parts {
        out.write_all(part)
            .map_err(|e| format!("{}: {e}", out_path.display()))?;
    }
    // Exit without running destructors: freeing the posting lists would only add to the timed
    // wall. `File` keeps no user-space buffer, so every byte is already written.
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
fn answer_all(query_text: &[u8], docs: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if docs.last().is_some_and(|&b| b != b'\n') {
        return Err("docs.txt: the last line has no LF (spec §2.1)".into());
    }
    let (queries, terms) = parse_queries(query_text)?;
    let plan = Plan::new(&queries, terms, docs);
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let bounds = line_bounds(docs, threads);
    // Line `i` holds id `i` (spec §2.1), so the id at a chunk's first line numbers all of its
    // lines, and the id after the last line is the number of documents.
    let firsts = (bounds.iter())
        .map(|&at| id_at(docs, at))
        .collect::<Result<Vec<_>, _>>()?;
    let (queries, plan) = (&queries, &plan);
    let chunks: Vec<Chunk> = thread::scope(|s| {
        let scanners: Vec<_> = (0..threads)
            .map(|c| {
                let (text, ids) = (&docs[bounds[c]..bounds[c + 1]], firsts[c]..firsts[c + 1]);
                s.spawn(move || scan(text, ids, queries, plan))
            })
            .collect();
        (scanners.into_iter())
            .map(|h| h.join().expect("a panic aborts the process"))
            .collect()
    });
    let (chunks, n) = (&chunks, queries.len());
    Ok(thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|t| n * t / threads..n * (t + 1) / threads)
            .map(|range| s.spawn(move || format_range(range, queries, plan, chunks)))
            .collect();
        (workers.into_iter())
            .map(|w| w.join().expect("a panic aborts the process"))
            .collect()
    }))
}

/// Splits `docs` at `parts - 1` line starts, so every line lands in exactly one chunk.
fn line_bounds(docs: &[u8], parts: usize) -> Vec<usize> {
    let mut bounds = vec![0];
    for i in 1..parts {
        let at = docs.len() * i / parts;
        let newline = docs[at..].iter().position(|&b| b == b'\n');
        bounds.push(newline.map_or(docs.len(), |p| at + p + 1));
    }
    bounds.push(docs.len());
    bounds
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    And,
    Or,
    Not,
}

impl Op {
    /// Every operator, indexed by its `as u32` value.
    const ALL: [Op; 3] = [Op::And, Op::Or, Op::Not];
}

struct Query {
    phrase: bool,
    /// Term ids in query order.
    terms: Vec<u32>,
    /// BOOL only: `ops[i]` combines the running result with `terms[i + 1]` (spec §3).
    ops: Vec<Op>,
}

/// Parses every line of `queries.txt` (grammar in spec §2.3). Duplicate lines stay separate
/// queries, since each one gets its own output line.
fn parse_queries<'q>(text: &'q [u8]) -> Result<(Vec<Query>, TermTable<'q>), String> {
    let mut table = TermTable::default();
    let mut list = Vec::new();
    for line in text.split_inclusive(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        // Rejecting anything but `[a-z0-9]{1,32}` keeps a term from matching a token that differs
        // from it only in case, which the case-folding key would otherwise allow, and keeps
        // every term within one `Key`.
        let mut term = |w: &'q [u8]| {
            let valid = (1..=MAX_TERM).contains(&w.len())
                && w.iter()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
            valid.then(|| table.intern(w))
        };
        let query = if let Some(rest) = line.strip_prefix(b"BOOL ") {
            // Terms sit at even positions and operators at odd ones, so a term spelled `and` is
            // an ordinary word and `AND` there is malformed.
            let words: Vec<&[u8]> = rest.split(|&b| b == b' ').collect();
            let terms: Option<Vec<u32>> = words.iter().step_by(2).map(|w| term(w)).collect();
            let ops: Option<Vec<Op>> = (words.iter().skip(1).step_by(2))
                .map(|w| match *w {
                    b"AND" => Some(Op::And),
                    b"OR" => Some(Op::Or),
                    b"NOT" => Some(Op::Not),
                    _ => None,
                })
                .collect();
            (terms.zip(ops))
                .filter(|(terms, ops)| terms.len() == ops.len() + 1)
                .map(|(terms, ops)| Query {
                    phrase: false,
                    terms,
                    ops,
                })
        } else if let Some(rest) =
            (line.strip_prefix(b"PHRASE \"")).and_then(|r| r.strip_suffix(b"\""))
        {
            let terms: Option<Vec<u32>> = rest.split(|&b| b == b' ').map(term).collect();
            terms.map(|terms| Query {
                phrase: true,
                terms,
                ops: Vec::new(),
            })
        } else {
            None
        };
        let bad = || {
            format!(
                "queries.txt: not a query: {}",
                String::from_utf8_lossy(line)
            )
        };
        list.push(query.ok_or_else(bad)?);
    }
    Ok((list, table))
}

/// How the corpus pass keeps a term's documents for the queries evaluated on whole sets.
#[derive(Clone, Copy)]
enum Store {
    /// Nothing is kept.
    None,
    /// Row `k` of each chunk's bitsets, one bit per document.
    Bits(u32),
    /// List `j` of each chunk's id lists.
    List(u32),
}

/// What the corpus pass needs to know about each query term.
struct Plan<'q> {
    /// The query terms, each with the [`Slot`] flags that say what the pass does with it.
    terms: TermTable<'q>,
    /// Per term: (query index, position in the phrase) of every phrase it triggers.
    phrase_triggers: Vec<Vec<(u32, u32)>>,
    /// Term `t` triggers the per-document checks `checks[check_at[t]..check_at[t + 1]]`.
    check_at: Vec<u32>,
    /// Checks of BOOL queries evaluated per document, back to back, each laid out as
    /// `q, n, s_1..s_n, m, op_1, t_1, .., op_m, t_m`. Query `q` matches a document that holds
    /// none of the cover terms `s_1..s_n` (a document holding one was checked from that term)
    /// and whose terms satisfy the operators, applied left to right starting from no match:
    /// `op_1` is OR. A check sits in one flat array so that running it reads no other memory.
    checks: Vec<u32>,
    /// Per term: how its documents are kept.
    store: Vec<Store>,
    /// The number of `Store::Bits` rows.
    rows: usize,
    /// The number of `Store::List` lists.
    lists: usize,
    /// Per query: evaluated on whole sets after the pass ([`on_sets`]) rather than per document.
    on_sets: Vec<bool>,
}

impl<'q> Plan<'q> {
    /// Picks every query's triggers and every term's [`Store`].
    ///
    /// A phrase match holds each of its terms at a fixed offset from the window start, so
    /// checking the windows around one term's occurrences finds all of them, and the rarest
    /// term costs the fewest checks. A BOOL query whose cover holds a common term would be
    /// evaluated on a large share of the documents, so it runs on sets instead: its common
    /// terms keep a bitset and the others an id list, whichever is smaller. A term absent
    /// from the corpus never fires and keeps an empty list, which leaves its queries at `0 0`
    /// with no special case.
    fn new(queries: &[Query], mut terms: TermTable<'q>, docs: &[u8]) -> Self {
        let (hits, sampled) = sample_hits(docs, &terms);
        // Fewer sample hits cost less. Among equals (often zero hits), longer terms tend to be
        // rarer, so they cost a little less.
        let cost = |t: u32| {
            let shorter = MAX_TERM - terms.name(t).len();
            u64::from(hits[t as usize]) * 64 + shorter as u64
        };
        // A bitset takes a bit per document and a list 32 bits per document holding the term.
        let common = |t: u32| hits[t as usize] as usize * DENSE_SHARE >= sampled.max(1);
        let mut flags = vec![0; terms.len()];
        let mut phrase_triggers = vec![Vec::new(); terms.len()];
        let mut checks_of = vec![Vec::new(); terms.len()];
        let mut store = vec![Store::None; terms.len()];
        let (mut rows, mut lists) = (0, 0);
        let mut on_sets = vec![false; queries.len()];
        for (i, query) in queries.iter().enumerate() {
            if query.phrase {
                let (offset, &term) = (query.terms.iter().enumerate())
                    .min_by_key(|&(_, &t)| cost(t))
                    .expect("the parser rejects a phrase without terms");
                phrase_triggers[term as usize].push((i as u32, offset as u32));
                flags[term as usize] |= Slot::PHRASE;
                continue;
            }
            let cover = bool_cover(query, cost);
            if !cover.iter().any(|&t| common(t)) {
                for (k, &term) in cover.iter().enumerate() {
                    let check: &mut Vec<u32> = &mut checks_of[term as usize];
                    check.extend([i as u32, k as u32]);
                    check.extend(&cover[..k]);
                    check.push(query.terms.len() as u32);
                    let ops = std::iter::once(&Op::Or).chain(&query.ops);
                    for (&op, &t) in ops.zip(&query.terms) {
                        check.extend([op as u32, t]);
                    }
                    flags[term as usize] |= Slot::BOOL;
                }
                continue;
            }
            on_sets[i] = true;
            for &t in &query.terms {
                if let Store::None = store[t as usize] {
                    store[t as usize] = if common(t) {
                        rows += 1;
                        Store::Bits(rows - 1)
                    } else {
                        lists += 1;
                        Store::List(lists - 1)
                    };
                    flags[t as usize] |= Slot::STORED;
                }
            }
        }
        let mut check_at = vec![0];
        let mut checks = Vec::new();
        for check in checks_of {
            checks.extend(check);
            check_at.push(checks.len() as u32);
        }
        terms.mark(&flags);
        Self {
            terms,
            phrase_triggers,
            check_at,
            checks,
            store,
            rows: rows as usize,
            lists: lists as usize,
            on_sets,
        }
    }
}

/// Returns terms such that every document matching the BOOL `query` contains one of them.
///
/// Read left to right, the first term covers `S(t1)`. `AND t` leaves a subset of both sides,
/// so the cheaper of the running cover and `t` alone will do. `OR t` adds `S(t)`, so `t`
/// joins the cover. `NOT t` only removes documents, so the cover stands. For
/// `estate OR surety AND bail` that is `{estate, surety}` or `{bail}`, whichever costs less.
fn bool_cover(query: &Query, cost: impl Fn(u32) -> u64) -> Vec<u32> {
    let mut cover = vec![query.terms[0]];
    for (&op, &term) in query.ops.iter().zip(&query.terms[1..]) {
        match op {
            Op::And if cost(term) < cover.iter().map(|&t| cost(t)).sum() => cover = vec![term],
            Op::Or => cover.push(term),
            Op::And | Op::Not => {}
        }
    }
    cover.sort_unstable();
    cover.dedup();
    cover
}

impl Plan<'_> {
    /// The per-document checks that term `t` triggers, laid out as in [`Plan::checks`].
    fn checks(&self, t: u32) -> &[u32] {
        let t = t as usize;
        &self.checks[self.check_at[t] as usize..self.check_at[t + 1] as usize]
    }
}

/// Runs the per-document `checks` ([`Plan::checks`]) on a document that holds the terms for
/// which `has` is true, and calls `matched` with every query that matches it.
///
/// On a single document the set operations become boolean ones, applied strictly left to
/// right (spec §3): `AND` is `&&`, `OR` is `||` and `NOT` is `&& !`.
fn run_checks(mut checks: &[u32], has: impl Fn(u32) -> bool, mut matched: impl FnMut(u32)) {
    while let [q, skips, rest @ ..] = checks {
        let (skip, rest) = rest.split_at(*skips as usize);
        let [steps, rest @ ..] = rest else {
            unreachable!("Plan::new writes whole checks")
        };
        let (steps, rest) = rest.split_at(2 * *steps as usize);
        checks = rest;
        if skip.iter().any(|&t| has(t)) {
            continue;
        }
        let steps = steps.as_chunks::<2>().0.iter();
        let result = steps.fold(false, |result, &[op, t]| match Op::ALL[op as usize] {
            Op::And => result && has(t),
            Op::Or => result || has(t),
            Op::Not => result && !has(t),
        });
        if result {
            matched(*q);
        }
    }
}

/// Counts, for each term, the documents among the first `SAMPLE_DOCS` that contain it, and
/// returns those counts with the number of documents sampled: a cheap estimate of document
/// frequency.
fn sample_hits(docs: &[u8], terms: &TermTable) -> (Vec<u32>, usize) {
    let mut lines = docs.iter().enumerate().filter(|&(_, &b)| b == b'\n');
    let end = (lines.nth(SAMPLE_DOCS - 1)).map_or(docs.len(), |(at, _)| at + 1);
    let sample = &docs[..end];
    let mut hits = vec![0; terms.len()];
    // Per term, the last sampled document counted, so a document counts once.
    let mut counted = vec![usize::MAX; terms.len()];
    let mut doc = 0;
    walk(sample, |event| match event {
        Event::Token { start, end } => {
            if let Some(slot) = key_in(sample, start, end).and_then(|key| terms.find(&key))
                && counted[slot.id as usize] != doc
            {
                counted[slot.id as usize] = doc;
                hits[slot.id as usize] += 1;
            }
        }
        Event::LineEnd => doc += 1,
    });
    (hits, doc)
}

/// The id of the line that starts at `at`, or the number of documents when `at` is the end of
/// `docs`. Line `i` holds id `i` (spec §2.1), so the ids of the lines after it follow by
/// counting.
fn id_at(docs: &[u8], at: usize) -> Result<u32, String> {
    if at == docs.len() {
        let Some(body) = docs.strip_suffix(b"\n") else {
            return Ok(0); // an empty docs.txt
        };
        let last_line = body
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |lf| lf + 1);
        return Ok(id_at(docs, last_line)? + 1);
    }
    let digits = docs[at..].iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || docs.get(at + digits) != Some(&b'\t') {
        let start = String::from_utf8_lossy(&docs[at..docs.len().min(at + 40)]);
        return Err(format!(
            "docs.txt: no id and TAB at the line starting {start:?}"
        ));
    }
    Ok((docs[at..at + digits].iter()).fold(0, |id, &digit| id * 10 + u32::from(digit - b'0')))
}

/// What [`walk`] finds in a chunk of `docs.txt`, in input order.
enum Event {
    /// A token of a document's text, at `chunk[start..end]`.
    Token { start: usize, end: usize },
    /// The LF that ends a document's line.
    LineEnd,
}

/// Calls `on` with every token of every document in `chunk`, and with every line end, in
/// order. `chunk` holds whole lines, the last one ending in LF.
///
/// No loop here looks at single bytes. [`classify`] turns 64 bytes at a time into two masks:
/// `token`, with bit `i` set when byte `i` is a letter or digit, and `lf`. A token starts where
/// a token byte follows a separator and ends at the separator after its last byte. With `prev`
/// the `token` mask shifted by one byte (the previous block's last bit shifted in), that is:
///
/// ```text
/// bytes    T h e   s u r e t y ,   b a i l
/// token    1 1 1 0 1 1 1 1 1 1 0 0 1 1 1 1
/// starts   1 0 0 0 1 0 0 0 0 0 0 0 1 0 0 0    token & !prev
/// ends     0 0 0 1 0 0 0 0 0 0 1 0 0 0 0 0    !token & prev
/// ```
///
/// `trailing_zeros` walks the set bits. Starts and ends alternate, so each end closes the
/// oldest open start, and a token still open at a block's end carries into the next block.
/// A line starts with its id, digits that look like a token, so a token that starts a line is
/// skipped: document text only begins after the TAB.
fn walk(chunk: &[u8], mut on: impl FnMut(Event)) {
    let mut prev_bit = 0; // 1 when the byte before the block is a token byte
    let mut open = None; // the start of a token that runs on past the block
    let mut line_start = 0;
    let mut padded = [0; 64]; // zero bytes are separators
    for base in (0..chunk.len()).step_by(64) {
        let block = if let Some(block) = chunk[base..].first_chunk() {
            block
        } else {
            padded[..chunk.len() - base].copy_from_slice(&chunk[base..]);
            &padded
        };
        // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
        let (token, lf) = unsafe { classify(block) };
        let prev = token << 1 | prev_bit;
        let (mut starts, mut ends, mut lfs) = (token & !prev, !token & prev, lf);
        prev_bit = token >> 63;
        while ends != 0 {
            let end = base + ends.trailing_zeros() as usize;
            ends &= ends - 1;
            let start = open.take().unwrap_or_else(|| {
                let start = base + starts.trailing_zeros() as usize;
                starts &= starts - 1;
                start
            });
            while lfs != 0 && base + (lfs.trailing_zeros() as usize) < start {
                line_start = base + lfs.trailing_zeros() as usize + 1;
                lfs &= lfs - 1;
                on(Event::LineEnd);
            }
            if start != line_start {
                on(Event::Token { start, end });
            }
        }
        if starts != 0 {
            open = Some(base + starts.trailing_zeros() as usize);
        }
        while lfs != 0 {
            line_start = base + lfs.trailing_zeros() as usize + 1;
            lfs &= lfs - 1;
            on(Event::LineEnd);
        }
    }
}

/// Returns `(token, lf)` for a 64-byte block: bit `i` of `token` is set when `block[i]` is a
/// token byte, an ASCII letter or digit (spec §2.2), and bit `i` of `lf` when it is an LF.
///
/// Each half of the block is one AVX2 register. A byte is a letter when `(b | 0x20) - b'a'`
/// is at most 25 as an unsigned byte, and a digit when `b - b'0'` is at most 9, so every byte
/// from 0x80 up, TAB and LF included, is a separator. `_mm256_movemask_epi8` then packs the
/// top bit of each byte's 0x00 or 0xFF result into one bit.
#[target_feature(enable = "avx2")]
fn classify(block: &[u8; 64]) -> (u64, u64) {
    use std::arch::x86_64::{
        _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_min_epu8, _mm256_movemask_epi8,
        _mm256_or_si256, _mm256_set1_epi8, _mm256_sub_epi8,
    };
    let (mut token, mut lf) = (0, 0);
    for offset in [0, 32] {
        // SAFETY: the 32-byte unaligned load lies inside the 64-byte block.
        let bytes = unsafe { _mm256_loadu_si256(block.as_ptr().add(offset).cast()) };
        let lower = _mm256_or_si256(bytes, _mm256_set1_epi8(0x20));
        let letter = _mm256_sub_epi8(lower, _mm256_set1_epi8(b'a'.cast_signed()));
        let letter = _mm256_cmpeq_epi8(_mm256_min_epu8(letter, _mm256_set1_epi8(25)), letter);
        let digit = _mm256_sub_epi8(bytes, _mm256_set1_epi8(b'0'.cast_signed()));
        let digit = _mm256_cmpeq_epi8(_mm256_min_epu8(digit, _mm256_set1_epi8(9)), digit);
        let newline = _mm256_cmpeq_epi8(bytes, _mm256_set1_epi8(b'\n'.cast_signed()));
        let is_token = _mm256_movemask_epi8(_mm256_or_si256(letter, digit)).cast_unsigned();
        token |= u64::from(is_token) << offset;
        lf |= u64::from(_mm256_movemask_epi8(newline).cast_unsigned()) << offset;
    }
    (token, lf)
}

/// One chunk's answers to the queries evaluated per document: how many of its documents match,
/// the sum of their ids, and the first `SHOWN_IDS` of them.
struct Tally {
    count: Vec<u32>,
    sum: Vec<u64>,
    /// (query index, doc id) of each query's first matches in the chunk, in doc order.
    first: Vec<(u32, u32)>,
}

impl Tally {
    fn new(queries: usize) -> Self {
        Self {
            count: vec![0; queries],
            sum: vec![0; queries],
            first: Vec::new(),
        }
    }

    /// Records that `doc` matches query `q`. Docs arrive in id order, so the first matches a
    /// chunk records are its smallest ids.
    fn add(&mut self, q: u32, doc: u32) {
        let i = q as usize;
        self.count[i] += 1;
        self.sum[i] += u64::from(doc);
        if self.count[i] as usize <= SHOWN_IDS {
            self.first.push((q, doc));
        }
    }

    /// Takes the first matches out, grouped by query and each in doc order: query `q`'s are
    /// `ids[at[q]..at[q + 1]]`. `count` says how many each query has, so this is a counting
    /// sort, run by each scanning thread instead of one big sort after the pass.
    fn take_first(&mut self) -> (Vec<u32>, Vec<u32>) {
        let mut at = vec![0];
        at.extend(self.count.iter().scan(0, |end, &n| {
            *end += n.min(SHOWN_IDS as u32);
            Some(*end)
        }));
        let (mut next, mut ids) = (at.clone(), vec![0; self.first.len()]);
        for (q, doc) in std::mem::take(&mut self.first) {
            ids[next[q as usize] as usize] = doc;
            next[q as usize] += 1;
        }
        (at, ids)
    }
}

/// What the corpus pass keeps from one chunk of whole lines.
struct Chunk {
    /// The chunk's document ids.
    ids: Range<u32>,
    tally: Tally,
    /// Each query's first matches in the chunk, smallest first: query `q`'s are
    /// `shown[shown_at[q]..shown_at[q + 1]]`.
    shown_at: Vec<u32>,
    shown: Vec<u32>,
    /// `Store::Bits` rows of `words()` words each: bit `i` of row `k` is set when document
    /// `ids.start + i` contains term `k`.
    bits: Vec<u64>,
    /// `Store::List` lists: the ascending ids of the chunk's documents that contain the term.
    lists: Vec<Vec<u32>>,
}

impl Chunk {
    fn words(&self) -> usize {
        ((self.ids.end - self.ids.start) as usize).div_ceil(64)
    }

    /// The chunk's documents that contain a term with this `store`, as bitset words. A list is
    /// spread into `scratch` first.
    fn row<'a>(&'a self, store: Store, scratch: &'a mut Vec<u64>) -> &'a [u64] {
        let words = self.words();
        match store {
            Store::Bits(k) => &self.bits[k as usize * words..][..words],
            Store::List(j) => {
                scratch.clear();
                scratch.resize(words, 0);
                for &doc in &self.lists[j as usize] {
                    let i = (doc - self.ids.start) as usize;
                    scratch[i / 64] |= 1 << (i % 64);
                }
                scratch
            }
            Store::None => unreachable!("every term of a query on sets has a store"),
        }
    }
}

/// Runs the corpus pass over `text`, whole lines holding the documents `ids`.
///
/// A token costs one probe of the term table, whose [`Slot`] says what to do with the term,
/// and otherwise only touches the current document's state: which terms it contains (one bit
/// each, small enough to stay in L1), and the bitset rows and lists of terms that keep them.
/// At each line end, the phrases triggered in the document are checked window by window, and
/// the per-document checks of the BOOL terms it contains are run ([`run_checks`]).
fn scan(text: &[u8], ids: Range<u32>, queries: &[Query], plan: &Plan) -> Chunk {
    let words = ((ids.end - ids.start) as usize).div_ceil(64);
    let mut bits = vec![0u64; plan.rows * words];
    let mut lists = vec![Vec::new(); plan.lists];
    let mut tally = Tally::new(queries.len());
    // Bit `t` is set while the current doc contains term `t`.
    let mut seen = vec![0u64; plan.terms.len().div_ceil(64)];
    // Per phrase, the last doc it matched, so no doc counts twice.
    let mut decided = vec![u32::MAX; queries.len()];
    // The current doc's tokens as (key, home slot), probed only at the line end: by then every
    // slot load is long under way ([`TermTable::prefetch`]), instead of each stalling in turn.
    let mut probes: Vec<Option<(Key, usize)>> = Vec::new();
    // The current doc's term id per token (MISS for the rest), the positions of its phrase
    // triggers, and its distinct BOOL triggers.
    let (mut terms, mut phrase_at, mut bool_terms) = (Vec::new(), Vec::new(), Vec::new());
    let mut doc = ids.start;
    walk(text, |event| match event {
        Event::Token { start, end } => {
            let key = key_in(text, start, end);
            probes.push(key.map(|key| (key, plan.terms.prefetch(&key))));
        }
        Event::LineEnd => {
            for probe in probes.drain(..) {
                let Some(slot) = probe.and_then(|(key, at)| plan.terms.find_from(&key, at)) else {
                    terms.push(MISS);
                    continue;
                };
                let (t, bit) = (slot.id as usize / 64, 1 << (slot.id % 64));
                if seen[t] & bit == 0 {
                    seen[t] |= bit;
                    if slot.flags & Slot::STORED != 0 {
                        match plan.store[slot.id as usize] {
                            Store::Bits(k) => {
                                let i = (doc - ids.start) as usize;
                                bits[k as usize * words + i / 64] |= 1 << (i % 64);
                            }
                            Store::List(j) => lists[j as usize].push(doc),
                            Store::None => unreachable!("Plan::new flags only stored terms"),
                        }
                    }
                    if slot.flags & Slot::BOOL != 0 {
                        bool_terms.push(slot.id);
                    }
                }
                if slot.flags & Slot::PHRASE != 0 {
                    phrase_at.push(terms.len());
                }
                terms.push(slot.id);
            }
            for &at in &phrase_at {
                for &(q, offset) in &plan.phrase_triggers[terms[at] as usize] {
                    let phrase: &[u32] = &queries[q as usize].terms;
                    // `terms` holds this doc only, so a window never spans two docs (spec §3).
                    let start = at.checked_sub(offset as usize);
                    let window = start.and_then(|s| terms.get(s..s + phrase.len()));
                    if decided[q as usize] != doc && window == Some(phrase) {
                        decided[q as usize] = doc;
                        tally.add(q, doc);
                    }
                }
            }
            let has = |t: u32| seen[t as usize / 64] & 1 << (t % 64) != 0;
            for &term in &bool_terms {
                run_checks(plan.checks(term), has, |q| tally.add(q, doc));
            }
            for &t in terms.iter().filter(|&&t| t != MISS) {
                seen[t as usize / 64] &= !(1 << (t % 64));
            }
            terms.clear();
            phrase_at.clear();
            bool_terms.clear();
            doc += 1;
        }
    });
    let (shown_at, shown) = tally.take_first();
    Chunk {
        ids,
        tally,
        shown_at,
        shown,
        bits,
        lists,
    }
}

/// Formats the answers to queries `range`, in order (spec §4).
fn format_range(range: Range<usize>, queries: &[Query], plan: &Plan, chunks: &[Chunk]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut scratch, mut shown) = (Vec::new(), Vec::new(), Vec::new());
    for q in range {
        shown.clear();
        let (count, sum) = if plan.on_sets[q] {
            on_sets(
                &queries[q],
                plan,
                chunks,
                (&mut acc, &mut scratch),
                &mut shown,
            )
        } else {
            // Chunks hold ascending ids, so the first chunks' first matches are the smallest.
            for c in chunks {
                let ids = &c.shown[c.shown_at[q] as usize..c.shown_at[q + 1] as usize];
                shown.extend(ids.iter().take(SHOWN_IDS - shown.len()));
            }
            let count = chunks.iter().map(|c| u64::from(c.tally.count[q])).sum();
            (count, chunks.iter().map(|c| c.tally.sum[q]).sum())
        };
        write!(out, "{count} {sum}").expect("writing to a Vec cannot fail");
        for id in &shown {
            write!(out, " {id}").expect("writing to a Vec cannot fail");
        }
        out.push(b'\n');
    }
    out
}

/// Evaluates a BOOL query on whole sets, chunk by chunk and strictly left to right (spec §3).
/// Returns the count and the id sum, and puts the first `SHOWN_IDS` ids in `shown`.
///
/// On bitsets, AND, OR and NOT are the word-wise `&`, `|` and `& !`, 64 documents per
/// operation. The count is a popcount, and the id sum of a word at `base` is `base` times its
/// popcount plus the sum of its set bits' positions ([`bit_index_sum`]).
fn on_sets(
    query: &Query,
    plan: &Plan,
    chunks: &[Chunk],
    (acc, scratch): (&mut Vec<u64>, &mut Vec<u64>),
    shown: &mut Vec<u32>,
) -> (u64, u64) {
    let (mut count, mut sum) = (0, 0);
    for chunk in chunks {
        acc.clear();
        acc.extend_from_slice(chunk.row(plan.store[query.terms[0] as usize], scratch));
        for (&op, &term) in query.ops.iter().zip(&query.terms[1..]) {
            let pairs = acc
                .iter_mut()
                .zip(chunk.row(plan.store[term as usize], scratch));
            match op {
                Op::And => pairs.for_each(|(a, &w)| *a &= w),
                Op::Or => pairs.for_each(|(a, &w)| *a |= w),
                Op::Not => pairs.for_each(|(a, &w)| *a &= !w),
            }
        }
        for (i, &word) in acc.iter().enumerate() {
            let base = u64::from(chunk.ids.start) + 64 * i as u64;
            count += u64::from(word.count_ones());
            sum += base * u64::from(word.count_ones()) + bit_index_sum(word);
            let mut rest = word;
            while rest != 0 && shown.len() < SHOWN_IDS {
                shown.push((base + u64::from(rest.trailing_zeros())) as u32);
                rest &= rest - 1;
            }
        }
    }
    (count, sum)
}

/// The sum of the positions of the set bits in `word`. Mask `k` keeps the bits whose position
/// has bit `k` set, so each of its set bits adds `2^k` to the sum.
fn bit_index_sum(word: u64) -> u64 {
    const MASKS: [u64; 6] = [
        0xAAAA_AAAA_AAAA_AAAA,
        0xCCCC_CCCC_CCCC_CCCC,
        0xF0F0_F0F0_F0F0_F0F0,
        0xFF00_FF00_FF00_FF00,
        0xFFFF_0000_FFFF_0000,
        0xFFFF_FFFF_0000_0000,
    ];
    (MASKS.iter().enumerate())
        .map(|(k, &mask)| u64::from((word & mask).count_ones()) << k)
        .sum()
}

/// A token's bytes, ASCII-lowercased and zero-padded to `MAX_TERM`, as little-endian words.
/// A token never holds a zero byte, so two keys are equal exactly when the tokens are.
type Key = [u64; MAX_TERM / 8];
/// The longest query term (spec §2.3), and so the longest token worth looking up.
const MAX_TERM: usize = 32;

/// The key of a token or query term, or `None` past `MAX_TERM` bytes: no query term is longer.
fn key(token: &[u8]) -> Option<Key> {
    let mut bytes = [0; MAX_TERM];
    bytes.get_mut(..token.len())?.copy_from_slice(token);
    Some(fold(&bytes, token.len()))
}

/// The key of the token at `chunk[start..end]`, read as one 32-byte load from `chunk` and
/// masked to the token's length, or `None` past `MAX_TERM` bytes. Copying the token into a
/// zeroed buffer first would call `memcpy` and then stall the load that reads the buffer
/// back, once per token.
fn key_in(chunk: &[u8], start: usize, end: usize) -> Option<Key> {
    match chunk[start..].first_chunk() {
        Some(window) => (end - start <= MAX_TERM).then(|| fold(window, end - start)),
        None => key(&chunk[start..end]), // within 32 bytes of the chunk's end
    }
}

/// Lowercases the first `len` bytes of `window`, all letters and digits, and zeroes the rest.
fn fold(window: &[u8; MAX_TERM], len: usize) -> Key {
    std::array::from_fn(|i| {
        let word = u64::from_le_bytes(*window[8 * i..].first_chunk().expect("8 of 32 bytes"));
        let kept = len.saturating_sub(8 * i).min(8);
        let word = word & ((1u128 << (8 * kept)) - 1) as u64;
        // Copying each byte's bit 6 into its bit 5 lowercases a letter (A-Z have bit 6 set)
        // and leaves digits and the zero padding alone.
        word | (word >> 1 & 0x2020_2020_2020_2020)
    })
}

/// Multiplicative hash of a key. Slots come from its top bits, which depend on every word.
fn key_hash(key: &Key) -> u64 {
    (key.iter()).fold(0, |hash, &word| {
        (hash ^ word).wrapping_mul(0x9e37_79b9_7f4a_7c15)
    })
}

/// One slot of a [`TermTable`]: a term's first key word, its id and what the corpus pass does
/// with it, 16 bytes that one load brings in.
#[derive(Clone, Copy, Default)]
struct Slot {
    /// `key[0]`, the term's first 8 bytes; 0 marks an empty slot, as a term's first byte is
    /// never zero.
    head: u64,
    /// The term id.
    id: u32,
    /// `LONG`, plus the [`Plan`]'s flags for the term.
    flags: u32,
}

impl Slot {
    /// The term runs past 8 bytes, so `head` alone does not identify it.
    const LONG: u32 = 1;
    /// The term triggers phrase checks.
    const PHRASE: u32 = 2;
    /// The term triggers per-document BOOL checks.
    const BOOL: u32 = 4;
    /// The corpus pass keeps the term's documents ([`Store`]).
    const STORED: u32 = 8;
}

/// Open-addressing map from query term to its [`Slot`]. A probe compares a slot's `head` with
/// the token's first key word; only a term longer than 8 bytes, rare among tokens, then loads
/// the rest of its key. So most tokens cost one memory access here, whether or not they are
/// query terms. It doubles at half load, so it assumes nothing about how many distinct terms
/// the queries use.
struct TermTable<'q> {
    /// The slot count is a power of two.
    slots: Vec<Slot>,
    /// Term id → its key.
    keys: Vec<Key>,
    /// Term id → the term as written in `queries.txt`.
    names: Vec<&'q [u8]>,
}

impl Default for TermTable<'_> {
    fn default() -> Self {
        Self {
            slots: vec![Slot::default(); 1024],
            keys: Vec::new(),
            names: Vec::new(),
        }
    }
}

impl<'q> TermTable<'q> {
    fn len(&self) -> usize {
        self.names.len()
    }

    fn name(&self, id: u32) -> &'q [u8] {
        self.names[id as usize]
    }

    /// The slot of the query term whose key this is, if any.
    fn find(&self, key: &Key) -> Option<Slot> {
        self.find_from(key, self.home(key))
    }

    /// [`TermTable::find`], given `at = self.home(key)`.
    fn find_from(&self, key: &Key, at: usize) -> Option<Slot> {
        let slot = self.slots[self.position(key, at)];
        (slot.head != 0).then_some(slot)
    }

    /// Where the probe for `key` starts.
    fn home(&self, key: &Key) -> usize {
        (key_hash(key) >> (64 - self.slots.len().trailing_zeros())) as usize
    }

    /// Returns [`TermTable::home`] and starts loading that slot into L1, so that the load
    /// overlaps other work until [`TermTable::find_from`] needs it.
    fn prefetch(&self, key: &Key) -> usize {
        use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
        let at = self.home(key);
        // SAFETY: a prefetch never faults or changes memory, and this address is in `slots`.
        unsafe { _mm_prefetch::<_MM_HINT_T0>(self.slots[at..].as_ptr().cast()) };
        at
    }

    fn intern(&mut self, name: &'q [u8]) -> u32 {
        let key = key(name).expect("the parser rejects terms over MAX_TERM bytes");
        let at = self.position(&key, self.home(&key));
        if self.slots[at].head != 0 {
            return self.slots[at].id;
        }
        let id = self.names.len() as u32;
        let flags = if key[1] != 0 { Slot::LONG } else { 0 };
        self.slots[at] = Slot {
            head: key[0],
            id,
            flags,
        };
        self.keys.push(key);
        self.names.push(name);
        if self.names.len() * 2 > self.slots.len() {
            let grown = vec![Slot::default(); self.slots.len() * 2];
            let old = std::mem::replace(&mut self.slots, grown);
            for slot in old.into_iter().filter(|slot| slot.head != 0) {
                let key = &self.keys[slot.id as usize];
                let at = self.position(key, self.home(key));
                self.slots[at] = slot;
            }
        }
        id
    }

    /// Adds `flags[id]` to the flags of each term `id`.
    fn mark(&mut self, flags: &[u32]) {
        for slot in self.slots.iter_mut().filter(|slot| slot.head != 0) {
            slot.flags |= flags[slot.id as usize];
        }
    }

    /// The slot holding `key`, or the empty slot where it belongs, probing linearly from `at`.
    fn position(&self, key: &Key, mut at: usize) -> usize {
        let mask = self.slots.len() - 1;
        let long = key[1] != 0;
        loop {
            let slot = &self.slots[at];
            let found = slot.head == key[0]
                && (slot.flags & Slot::LONG != 0) == long
                && (!long || self.keys[slot.id as usize] == *key);
            if slot.head == 0 || found {
                return at;
            }
            at = (at + 1) & mask;
        }
    }
}
