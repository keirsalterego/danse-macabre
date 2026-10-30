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
//!    phrase at each position of its trigger term, a BOOL query by applying its operators to
//!    "does this document contain the term" ([`Check`]). Set operations act on each
//!    document separately, so this gives the same answer as combining whole sets (spec §3).
//!    For the queries on sets, the pass records which documents of the current [`Window`]
//!    hold their terms, a bitset per common term and an id list per rare one, and evaluates
//!    the queries at the window's end.
//! 4. **Merge.** Per query, the chunks' counts and id sums add up, and the smallest ids come
//!    from the earliest chunks ([`Tally`]).
//!
//! Memory stays at a few arrays per thread: per query a count, an id sum and the first
//! matches, and per kept term one window's documents. Numbers: PERF.md.

use std::alloc::Layout;
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
/// An output line lists at most this many ids (spec §4).
const SHOWN_IDS: usize = 10;
/// The low bits of a [`Tally`] cell hold the sum of the ids matched in a chunk, which
/// [`answer_all`] keeps below `2^SUM_BITS`; the high bits hold their count.
const SUM_BITS: u32 = 40;
/// A term is common when at least one sampled document in this many holds it: as a list of
/// 16-bit offsets ([`Window`]), it would then take at least half the memory of its bitset.
const DENSE_SHARE: usize = 32;
/// The queries on sets are evaluated every `64 * WINDOW` documents, on bitsets of `WINDOW`
/// words ([`Window`]). Its id lists hold 16-bit offsets, so it spans at most 2^16 documents.
const WINDOW: usize = 64;
const _: () = assert!(64 * WINDOW <= 1 << 16);

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn mallopt(param: i32, value: i32) -> i32;
    fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
}
const PROT_READ: i32 = 1;
const MAP_PRIVATE: i32 = 2;
const M_MMAP_THRESHOLD: i32 = -3;
const MADV_HUGEPAGE: i32 = 14;

fn main() -> Result<(), Box<dyn Error>> {
    // glibc raises its mmap threshold to the largest block freed so far, and blocks below it
    // then stay resident after they are freed. Fixed, every block of 128 KiB or more goes back
    // to the kernel when freed, such as each thread's first matches once they are grouped.
    // SAFETY: no other thread exists yet, and mallopt takes no pointers.
    unsafe { mallopt(M_MMAP_THRESHOLD, 128 << 10) };
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
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let (queries, terms) = parse_queries(query_text, threads)?;
    let plan = Plan::new(&queries, terms, docs);
    let bounds = line_bounds(docs, threads);
    // Line `i` holds id `i` (spec §2.1), so the id at a chunk's first line numbers all of its
    // lines, and the id after the last line is the number of documents.
    let firsts = (bounds.iter())
        .map(|&at| id_at(docs, at))
        .collect::<Result<Vec<_>, _>>()?;
    // A query's id sum over a chunk stays below the square of the document count.
    // shortcut: 2^20 or more documents stop here; the spec's full scale has 250,000. Widen the
    // cells to u128 if a corpus that large must run.
    if firsts
        .last()
        .is_some_and(|&n| u64::from(n).pow(2) >> SUM_BITS != 0)
    {
        return Err("docs.txt: too many documents".into());
    }
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
            .map(|range| s.spawn(move || format_range(range, chunks)))
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

/// The queries of `queries.txt` in file order, their terms in flat arrays. Duplicate lines stay
/// separate queries, since each one gets its own output line.
#[derive(Default)]
struct Queries {
    /// Per query: a phrase rather than a BOOL query.
    phrase: Vec<bool>,
    /// Query `q`'s terms are `terms[at[q]..at[q + 1]]`, in query order.
    at: Vec<u32>,
    /// Term ids.
    terms: Vec<u32>,
    /// `ops[i]` joins `terms[i]` to the result of the query's terms before it (spec §3). A
    /// query's first term joins the empty result with OR, and so does every phrase term.
    ops: Vec<Op>,
}

impl Queries {
    fn len(&self) -> usize {
        self.phrase.len()
    }

    /// Query `q`'s terms.
    fn terms(&self, q: usize) -> &[u32] {
        &self.terms[self.at[q] as usize..self.at[q + 1] as usize]
    }

    /// Query `q`'s terms, each with the operator that joins it.
    fn steps(&self, q: usize) -> impl Iterator<Item = (Op, u32)> {
        let ops = &self.ops[self.at[q] as usize..self.at[q + 1] as usize];
        ops.iter().copied().zip(self.terms(q).iter().copied())
    }
}

/// One thread's share of `queries.txt`, parsed as in [`Queries`] but with each term still as
/// its key.
#[derive(Default)]
struct Lines {
    phrase: Vec<bool>,
    /// Per query: how many terms it has.
    lens: Vec<u32>,
    ops: Vec<Op>,
    keys: Vec<Key>,
}

/// Parses `queries.txt` (grammar in spec §2.3). Every thread parses a share of the lines; then
/// one thread gives each distinct term its id, in file order, loading the table slot of the
/// term 16 ahead while it interns one, so that its cache misses overlap.
fn parse_queries(text: &[u8], threads: usize) -> Result<(Queries, TermTable), String> {
    let bounds = line_bounds(text, threads);
    let parts = thread::scope(|s| {
        let workers: Vec<_> = (bounds.windows(2))
            .map(|w| s.spawn(move || parse_lines(&text[w[0]..w[1]])))
            .collect();
        (workers.into_iter())
            .map(|w| w.join().expect("a panic aborts the process"))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let occurrences = parts.iter().map(|part| part.keys.len()).sum();
    let (mut queries, mut table) = (Queries::default(), TermTable::new(occurrences));
    queries.at.push(0);
    for part in parts {
        queries.phrase.extend(part.phrase);
        for len in part.lens {
            queries.at.push(queries.at[queries.at.len() - 1] + len);
        }
        queries.ops.extend(part.ops);
        for (i, key) in part.keys.iter().enumerate() {
            if let Some(ahead) = part.keys.get(i + 16) {
                table.prefetch(ahead);
            }
            queries.terms.push(table.intern(*key));
        }
    }
    Ok((queries, table))
}

/// Parses whole lines of `queries.txt`.
fn parse_lines(text: &[u8]) -> Result<Lines, String> {
    let mut lines = Lines::default();
    for line in text.split_inclusive(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let terms = lines.keys.len();
        if parse_line(line, &mut lines).is_none() {
            let line = String::from_utf8_lossy(line);
            return Err(format!("queries.txt: not a query: {line}"));
        }
        lines.lens.push((lines.keys.len() - terms) as u32);
    }
    Ok(lines)
}

/// Appends the query on `line` to `lines`, or returns `None` if the line is not one.
fn parse_line(line: &[u8], lines: &mut Lines) -> Option<()> {
    // Rejecting anything but `[a-z0-9]{1,32}` keeps a term from matching a token that differs
    // from it only in case, which the case-folding key would otherwise allow, and keeps every
    // term within one `Key`.
    let mut push = |op, term: &[u8]| {
        let valid = (1..=MAX_TERM).contains(&term.len())
            && term
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        lines.keys.push(key(term).filter(|_| valid)?);
        lines.ops.push(op);
        Some(())
    };
    let phrase = line
        .strip_prefix(b"PHRASE \"")
        .and_then(|r| r.strip_suffix(b"\""));
    if let Some(words) = phrase {
        for term in words.split(|&b| b == b' ') {
            push(Op::Or, term)?;
        }
    } else {
        // Terms sit at even positions and operators at odd ones, so a term spelled `and` is an
        // ordinary word and `AND` there is malformed.
        let mut words = line.strip_prefix(b"BOOL ")?.split(|&b| b == b' ');
        push(Op::Or, words.next()?)?;
        while let Some(op) = words.next() {
            let op = match op {
                b"AND" => Op::And,
                b"OR" => Op::Or,
                b"NOT" => Op::Not,
                _ => return None,
            };
            push(op, words.next()?)?;
        }
    }
    lines.phrase.push(phrase.is_some());
    Some(())
}

/// How the corpus pass keeps a term's documents for the queries evaluated on whole sets.
#[derive(Clone, Copy)]
enum Store {
    /// Nothing is kept.
    None,
    /// Row `k` of the [`Window`]'s bitsets, one bit per document.
    Bits(u32),
    /// List `j` of the window's id lists.
    List(u32),
}

/// What the corpus pass needs to know about each query term.
struct Plan {
    /// The query terms, each with the [`Slot`] flags that say what the pass does with it.
    terms: TermTable,
    /// Term `t` triggers the phrases `phrases[phrase_at[t]..phrase_at[t + 1]]`, each as (query
    /// index, position of the term in the phrase).
    phrase_at: Vec<u32>,
    phrases: Vec<(u32, u32)>,
    /// Term `t` triggers the per-document checks `checks[check_at[t]..check_at[t + 1]]`.
    check_at: Vec<u32>,
    checks: Vec<Check>,
    /// Per term: how its documents are kept.
    store: Vec<Store>,
    /// The number of `Store::Bits` rows.
    rows: usize,
    /// The number of `Store::List` lists.
    lists: usize,
    /// The queries evaluated on sets ([`Window::flush`]) rather than per document, in file order.
    sets: Vec<u32>,
}

impl Plan {
    /// Picks every query's triggers and every term's [`Store`].
    ///
    /// A phrase match holds each of its terms at a fixed offset from the window start, so
    /// checking the windows around one term's occurrences finds all of them, and the rarest
    /// term costs the fewest checks. A BOOL query whose cover holds a common term would be
    /// evaluated on a large share of the documents, so it runs on sets instead: its common
    /// terms keep a bitset and the others an id list, whichever is smaller. A term absent
    /// from the corpus never fires and keeps an empty list, which leaves its queries at `0 0`
    /// with no special case.
    fn new(queries: &Queries, mut terms: TermTable, docs: &[u8]) -> Self {
        let (hits, sampled) = sample_hits(docs, &terms);
        // Fewer sample hits cost less. Among equals (often zero hits), longer terms tend to be
        // rarer, so they cost a little less; a key's length is its count of nonzero bytes.
        // Computed in one pass, as reading hits and keys term by term costs two cache misses
        // per call.
        let costs: Vec<u64> = (hits.iter().zip(&terms.keys))
            .map(|(&hits, key)| {
                let len: u32 = key.iter().map(|word| 8 - word.leading_zeros() / 8).sum();
                u64::from(hits) * 64 + u64::from(MAX_TERM as u32 - len)
            })
            .collect();
        let cost = |t: u32| costs[t as usize];
        // A bitset takes a bit per document and a list 16 bits per document holding the term.
        let common = |t: u32| hits[t as usize] as usize * DENSE_SHARE >= sampled.max(1);
        let mut flags = vec![0; terms.len()];
        // (trigger term, phrase) and (trigger term, check word) pairs, in query order.
        let (mut phrases, mut checks) = (Vec::new(), Vec::new());
        let mut store = vec![Store::None; terms.len()];
        let (mut rows, mut lists) = (0, 0);
        let mut sets = Vec::new();
        let mut cover = Vec::new();
        for (i, &phrase) in queries.phrase.iter().enumerate() {
            let query = queries.terms(i);
            if phrase {
                let (offset, &term) = (query.iter().enumerate())
                    .min_by_key(|&(_, &t)| cost(t))
                    .expect("the parser rejects a phrase without terms");
                phrases.push((term, (i as u32, offset as u32)));
                flags[term as usize] |= Slot::PHRASE;
                continue;
            }
            bool_cover(queries.steps(i), cost, &mut cover);
            if !cover.iter().any(|&t| common(t))
                && let Some(check) = Check::new(i as u32, queries.steps(i), terms.len() as u32)
            {
                for (k, &term) in cover.iter().enumerate() {
                    // A document holding an earlier cover term was checked from that term.
                    checks.push((term, check.unless(&cover[..k])));
                    flags[term as usize] |= Slot::BOOL;
                }
                continue;
            }
            sets.push(i as u32);
            for &t in query {
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
        let (phrase_at, phrases) = group(terms.len(), &phrases);
        let (check_at, checks) = group(terms.len(), &checks);
        terms.mark(&flags);
        Self {
            terms,
            phrase_at,
            phrases,
            check_at,
            checks,
            store,
            rows: rows as usize,
            lists: lists as usize,
            sets,
        }
    }
}

/// Returns terms such that every document matching the BOOL `query` contains one of them.
///
/// Read left to right, the first term covers `S(t1)`. `AND t` leaves a subset of both sides,
/// so the cheaper of the running cover and `t` alone will do. `OR t` adds `S(t)`, so `t`
/// joins the cover. `NOT t` only removes documents, so the cover stands. For
/// `estate OR surety AND bail` that is `{estate, surety}` or `{bail}`, whichever costs less.
fn bool_cover(
    steps: impl Iterator<Item = (Op, u32)>,
    cost: impl Fn(u32) -> u64,
    cover: &mut Vec<u32>,
) {
    cover.clear();
    for (op, term) in steps {
        match op {
            Op::And if cost(term) < cover.iter().map(|&t| cost(t)).sum() => {
                cover.clear();
                cover.push(term);
            }
            Op::Or => cover.push(term),
            Op::And | Op::Not => {}
        }
    }
    cover.sort_unstable();
    cover.dedup();
}

impl Plan {
    /// The phrases that term `t` triggers, as (query index, position of `t` in the phrase).
    fn phrases(&self, t: u32) -> &[(u32, u32)] {
        let t = t as usize;
        &self.phrases[self.phrase_at[t] as usize..self.phrase_at[t + 1] as usize]
    }

    /// The indices of the per-document checks that term `t` triggers.
    fn check_range(&self, t: u32) -> Range<usize> {
        let t = t as usize;
        self.check_at[t] as usize..self.check_at[t + 1] as usize
    }
}

/// Groups `items` by key, keeping their order within a key: key `k`'s are
/// `grouped[at[k]..at[k + 1]]`. A counting sort, so it allocates no list per key.
fn group<T: Copy + Default>(keys: usize, items: &[(u32, T)]) -> (Vec<u32>, Vec<T>) {
    let mut at = vec![0; keys + 1];
    for &(k, _) in items {
        at[k as usize + 1] += 1;
    }
    for k in 0..keys {
        at[k + 1] += at[k];
    }
    let (mut next, mut grouped) = (at.clone(), vec![T::default(); items.len()]);
    for &(k, item) in items {
        grouped[next[k as usize] as usize] = item;
        next[k as usize] += 1;
    }
    (at, grouped)
}

/// A BOOL query compiled for one document at a time: its result as a truth table over which
/// of its distinct terms the document holds, so that checking it takes no branch.
#[derive(Clone, Copy, Default)]
struct Check {
    /// Bit `x` is the query's result on a document that holds `terms[j]` exactly for the bits
    /// `j` set in `x`.
    table: u64,
    q: u32,
    /// The query's distinct terms, padded with a term id that no document holds.
    terms: [u32; CHECK_TERMS],
}

/// The most distinct terms a [`Check`] takes; spec §2.3 allows 6 terms per BOOL query.
const CHECK_TERMS: usize = 6;

impl Check {
    /// Compiles query `q`, or returns `None` when it has too many distinct terms. `pad` is a
    /// term id that no document holds.
    ///
    /// On a single document the set operations become boolean ones, applied strictly left to
    /// right (spec §3): `AND` is `&&`, `OR` is `||` and `NOT` is `&& !`.
    ///
    /// Bit `x` of `HAS[j]` is set when input `x` holds `terms[j]`, so applying the operators
    /// to those masks evaluates the query on all 64 inputs at once.
    fn new(q: u32, steps: impl Iterator<Item = (Op, u32)>, pad: u32) -> Option<Self> {
        let (mut terms, mut n, mut table) = ([pad; CHECK_TERMS], 0, 0);
        for (op, t) in steps {
            let j = terms[..n].iter().position(|&u| u == t).unwrap_or(n);
            if j == n {
                *terms.get_mut(n)? = t;
                n += 1;
            }
            table = match op {
                Op::And => table & HAS[j],
                Op::Or => table | HAS[j],
                Op::Not => table & !HAS[j],
            };
        }
        Some(Self { table, q, terms })
    }

    /// This check, matching only documents that hold none of `skip`.
    fn unless(mut self, skip: &[u32]) -> Self {
        for (j, t) in self.terms.iter().enumerate() {
            if skip.contains(t) {
                self.table &= !HAS[j];
            }
        }
        self
    }

    /// Whether a document matches, given `has(t)`: 1 when it holds term `t`, else 0.
    fn matches(&self, has: impl Fn(u32) -> u64) -> bool {
        let x = (self.terms.iter().enumerate()).fold(0, |x, (j, &t)| x | has(t) << j);
        self.table >> x & 1 != 0
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
    let token = |&mut doc: &mut usize, start, end| {
        if let Some(key) = key_in(sample, start, end)
            && let Some(slot) = terms.find_from(&key, terms.home(&key))
            && counted[slot.id as usize] != doc
        {
            counted[slot.id as usize] = doc;
            hits[slot.id as usize] += 1;
        }
    };
    walk(sample, &mut doc, token, |doc| *doc += 1);
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

/// Calls `on_token(state, start, end)` with every token `chunk[start..end]` of every document
/// in `chunk`, and `on_line_end(state)` at every line end, in order. `chunk` holds whole lines,
/// the last one ending in LF. Two calls that share `state`, rather than one that takes either
/// event, let the compiler inline the call made once per token.
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
fn walk<S>(
    chunk: &[u8],
    state: &mut S,
    mut on_token: impl FnMut(&mut S, usize, usize),
    mut on_line_end: impl FnMut(&mut S),
) {
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
                on_line_end(state);
            }
            if start != line_start {
                on_token(state, start, end);
            }
        }
        if starts != 0 {
            open = Some(base + starts.trailing_zeros() as usize);
        }
        while lfs != 0 {
            line_start = base + lfs.trailing_zeros() as usize + 1;
            lfs &= lfs - 1;
            on_line_end(state);
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

/// One chunk's answers: how many of its documents match each query, the sum of their ids, and
/// the first `SHOWN_IDS` of them.
struct Tally {
    /// Per query, a cell: its matches as `count << SUM_BITS | id sum`, one load for both.
    queries: Vec<u64>,
    /// Per check ([`Plan::checks`]), a cell as above, next to its neighbours in the check list.
    checks: Vec<u64>,
    /// Per query, how many first matches its checks recorded: a byte, which stays in cache where
    /// the query's cell would not.
    shown: Vec<u8>,
    /// (query index, doc id) of each query's first matches in the chunk, in doc order.
    first: Vec<(u32, u32)>,
}

impl Tally {
    fn new(queries: usize, checks: usize) -> Self {
        Self {
            queries: vec![0; queries],
            checks: vec![0; checks],
            shown: vec![0; queries],
            first: Vec::new(),
        }
    }

    /// Records that `doc` matches query `q`. Docs arrive in id order, so the first matches a
    /// chunk records are its smallest ids.
    fn add(&mut self, q: u32, doc: u32) {
        let cell = &mut self.queries[q as usize];
        *cell += 1 << SUM_BITS | u64::from(doc);
        if *cell >> SUM_BITS <= SHOWN_IDS as u64 {
            self.first.push((q, doc));
        }
    }

    /// Records that `doc` matches check `c` of query `q` when `hit`, without a branch on
    /// `hit` in the common case: a check past its first matches only adds to its cell.
    fn add_check(&mut self, c: usize, q: u32, doc: u32, hit: bool) {
        let cell = &mut self.checks[c];
        // A conditional move: `hit` follows no pattern, and the compiler makes a product with
        // it a branch.
        *cell += std::hint::select_unpredictable(hit, 1 << SUM_BITS | u64::from(doc), 0);
        let shown = &mut self.shown[q as usize];
        if hit && *cell >> SUM_BITS <= SHOWN_IDS as u64 && usize::from(*shown) < SHOWN_IDS {
            *shown += 1;
            self.first.push((q, doc));
        }
    }

    /// Records the documents `base + i` for each bit `i` set in `word`, as [`Tally::add`] would
    /// one by one. Their count is a popcount, and their id sum is `base` times the count plus
    /// the sum of the set bits' positions ([`bit_index_sum`]).
    fn add_word(&mut self, q: u32, base: u32, mut word: u64) {
        while word != 0 && self.queries[q as usize] >> SUM_BITS < SHOWN_IDS as u64 {
            self.add(q, base + word.trailing_zeros());
            word &= word - 1;
        }
        let n = u64::from(word.count_ones());
        self.queries[q as usize] += n << SUM_BITS | (u64::from(base) * n + bit_index_sum(word));
    }

    /// Takes the first matches out, grouped by query and each in doc order: query `q`'s are
    /// `ids[at[q]..at[q + 1]]`. Each scanning thread groups its own, instead of one big sort
    /// after the pass.
    fn take_first(&mut self) -> (Vec<u32>, Vec<u32>) {
        group(self.queries.len(), &std::mem::take(&mut self.first))
    }
}

/// What the corpus pass keeps from one chunk of whole lines.
struct Chunk {
    tally: Tally,
    /// Each query's first matches in the chunk, smallest first: query `q`'s are
    /// `shown[shown_at[q]..shown_at[q + 1]]`.
    shown_at: Vec<u32>,
    shown: Vec<u32>,
}

/// The documents of one window of `64 * WINDOW` that contain the terms of the queries on sets
/// ([`Store`]). Only one window is kept at a time, so these sets take the same memory however
/// long the chunk is.
struct Window {
    /// The window's first document.
    start: u32,
    /// `Store::Bits` rows: bit `i` of row `k` is set when document `start + i` holds term `k`.
    bits: Vec<[u64; WINDOW]>,
    /// `Store::List` lists: the window's documents that contain the term, ascending, as
    /// offsets from `start`, which take half the memory of ids.
    lists: Vec<Vec<u16>>,
}

impl Window {
    /// Records that `doc` contains a term kept as `store`.
    fn add(&mut self, store: Store, doc: u32) {
        let i = (doc - self.start) as usize;
        match store {
            Store::Bits(k) => self.bits[k as usize][i / 64] |= 1 << (i % 64),
            Store::List(j) => self.lists[j as usize].push(i as u16),
            Store::None => unreachable!("Plan::new flags only stored terms"),
        }
    }

    /// The window's documents that contain a term kept as `store`, as a bitset. A list is
    /// spread into `scratch` first.
    fn row<'a>(&'a self, store: Store, scratch: &'a mut [u64; WINDOW]) -> &'a [u64; WINDOW] {
        match store {
            Store::Bits(k) => &self.bits[k as usize],
            Store::List(j) => {
                *scratch = [0; WINDOW];
                for &i in &self.lists[j as usize] {
                    scratch[usize::from(i / 64)] |= 1 << (i % 64);
                }
                scratch
            }
            Store::None => unreachable!("every term of a query on sets has a store"),
        }
    }

    /// Evaluates every query on sets over the window, strictly left to right (spec §3), adds
    /// the matches to `tally`, and moves on to the next window.
    ///
    /// On bitsets, AND, OR and NOT are the word-wise `&`, `|` and `& !`, 64 documents per
    /// operation.
    fn flush(&mut self, queries: &Queries, plan: &Plan, tally: &mut Tally) {
        let mut scratch = [0; WINDOW];
        for &q in &plan.sets {
            let mut acc = [0u64; WINDOW];
            for (op, term) in queries.steps(q as usize) {
                let row = self.row(plan.store[term as usize], &mut scratch);
                let pairs = acc.iter_mut().zip(row);
                match op {
                    Op::And => pairs.for_each(|(a, &w)| *a &= w),
                    Op::Or => pairs.for_each(|(a, &w)| *a |= w),
                    Op::Not => pairs.for_each(|(a, &w)| *a &= !w),
                }
            }
            for (i, &word) in acc.iter().enumerate() {
                tally.add_word(q, self.start + 64 * i as u32, word);
            }
        }
        self.bits.fill([0; WINDOW]);
        self.lists.iter_mut().for_each(Vec::clear);
        self.start += 64 * WINDOW as u32;
    }
}

/// Runs the corpus pass over `text`, whole lines holding the documents `ids`.
///
/// A token costs one probe of the term table, whose [`Slot`] says what to do with the term,
/// and otherwise only touches the current document's state: which terms it contains (one bit
/// each, small enough to stay in L1), and the [`Window`] of the terms that keep their
/// documents. At each line end, the phrases triggered in the document are checked at each
/// position of their trigger term, and the per-document checks of the BOOL terms it contains
/// are run ([`Check`]).
fn scan(text: &[u8], ids: Range<u32>, queries: &Queries, plan: &Plan) -> Chunk {
    let mut sets = Window {
        start: ids.start,
        bits: vec![[0; WINDOW]; plan.rows],
        lists: vec![Vec::new(); plan.lists],
    };
    let mut tally = Tally::new(queries.len(), plan.checks.len());
    // Bit `t` is set while the current doc contains term `t`. One more bit, never set, pads
    // the checks ([`Check`]) and stands for every token that is no query term, `miss`.
    let mut seen = vec![0u64; (plan.terms.len() + 1).div_ceil(64)];
    let miss = plan.terms.len() as u32;
    // The current doc's term id per token (`miss` for the rest), the positions of its phrase
    // triggers, the checks of its distinct BOOL triggers, and the phrases it matched, so none
    // counts twice.
    let (mut terms, mut phrase_at, mut bool_terms) = (Vec::new(), Vec::new(), Vec::new());
    let mut found = Vec::new();
    // The current doc's tokens as (key, home slot), probed only at the line end: by then every
    // slot load is long under way ([`TermTable::prefetch`]), instead of each stalling in turn.
    let mut probes: Vec<Option<(Key, usize)>> = Vec::new();
    let mut doc = ids.start;
    let token = |probes: &mut Vec<_>, start, end| {
        let key = key_in(text, start, end);
        probes.push(key.map(|key| (key, plan.terms.prefetch(&key))));
    };
    walk(text, &mut probes, token, |probes| {
        for probe in probes.drain(..) {
            let Some(slot) = probe.and_then(|(key, at)| plan.terms.find_from(&key, at)) else {
                terms.push(miss);
                continue;
            };
            let (t, bit) = (slot.id as usize / 64, 1 << (slot.id % 64));
            if seen[t] & bit == 0 {
                seen[t] |= bit;
                if slot.flags & Slot::STORED != 0 {
                    sets.add(plan.store[slot.id as usize], doc);
                }
                if slot.flags & Slot::BOOL != 0 {
                    // Start loading its checks and their cells now, to use at the line end.
                    let checks = plan.check_range(slot.id);
                    prefetch(&plan.checks[checks.start]);
                    prefetch(&tally.checks[checks.start]);
                    bool_terms.push(checks);
                }
            }
            if slot.flags & Slot::PHRASE != 0 {
                phrase_at.push(terms.len());
            }
            terms.push(slot.id);
        }
        for &at in &phrase_at {
            for &(q, offset) in plan.phrases(terms[at]) {
                let phrase = queries.terms(q as usize);
                // `terms` holds this doc only, so a match never spans two docs (spec §3).
                let start = at.checked_sub(offset as usize);
                let span = start.and_then(|s| terms.get(s..s + phrase.len()));
                if span == Some(phrase) && !found.contains(&q) {
                    found.push(q);
                    tally.add(q, doc);
                }
            }
        }
        let has = |t: u32| seen[t as usize / 64] >> (t % 64) & 1;
        for checks in bool_terms.drain(..) {
            for c in checks {
                let check = &plan.checks[c];
                tally.add_check(c, check.q, doc, check.matches(has));
            }
        }
        for &t in &terms {
            seen[t as usize / 64] &= !(1 << (t % 64));
        }
        terms.clear();
        phrase_at.clear();
        found.clear();
        doc += 1;
        if doc - sets.start == 64 * WINDOW as u32 {
            sets.flush(queries, plan, &mut tally);
        }
    });
    sets.flush(queries, plan, &mut tally);
    // The checks' cells hold their queries' whole counts and sums.
    for (check, cell) in plan.checks.iter().zip(std::mem::take(&mut tally.checks)) {
        tally.queries[check.q as usize] += cell;
    }
    let (shown_at, shown) = tally.take_first();
    Chunk {
        tally,
        shown_at,
        shown,
    }
}

/// Formats the answers to queries `range`, in order (spec §4).
fn format_range(range: Range<usize>, chunks: &[Chunk]) -> Vec<u8> {
    let (mut out, mut shown) = (Vec::new(), Vec::<u32>::new());
    for q in range {
        shown.clear();
        // Chunks hold ascending ids, so the first chunks' first matches are the smallest.
        for c in chunks {
            let ids = &c.shown[c.shown_at[q] as usize..c.shown_at[q + 1] as usize];
            shown.extend(ids.iter().take(SHOWN_IDS - shown.len()));
        }
        let count: u64 = chunks.iter().map(|c| c.tally.queries[q] >> SUM_BITS).sum();
        let sum: u64 = (chunks.iter())
            .map(|c| c.tally.queries[q] & ((1 << SUM_BITS) - 1))
            .sum();
        write!(out, "{count} {sum}").expect("writing to a Vec cannot fail");
        for id in &shown {
            write!(out, " {id}").expect("writing to a Vec cannot fail");
        }
        out.push(b'\n');
    }
    out
}

/// Mask `k` keeps the bits whose position has bit `k` set.
const HAS: [u64; 6] = [
    0xAAAA_AAAA_AAAA_AAAA,
    0xCCCC_CCCC_CCCC_CCCC,
    0xF0F0_F0F0_F0F0_F0F0,
    0xFF00_FF00_FF00_FF00,
    0xFFFF_0000_FFFF_0000,
    0xFFFF_FFFF_0000_0000,
];

/// The sum of the positions of the set bits in `word`: each set bit of mask `k` ([`HAS`])
/// adds `2^k`.
fn bit_index_sum(word: u64) -> u64 {
    (HAS.iter().enumerate())
        .map(|(k, &mask)| u64::from((word & mask).count_ones()) << k)
        .sum()
}

/// Starts loading the cache line that holds `item` into L1.
fn prefetch<T>(item: &T) {
    use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
    // SAFETY: a prefetch never faults or changes memory.
    unsafe { _mm_prefetch::<_MM_HINT_T0>(std::ptr::from_ref(item).cast()) };
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
    // `len` bytes of 0xFF, then zeros, are `ONES[MAX_TERM - len..]`.
    const ONES: [u8; 2 * MAX_TERM] = {
        let mut ones = [0; 2 * MAX_TERM];
        let mut i = 0;
        while i < MAX_TERM {
            ones[i] = 0xFF;
            i += 1;
        }
        ones
    };
    let mask: &[u8; MAX_TERM] = ONES[MAX_TERM - len..]
        .first_chunk()
        .expect("len <= MAX_TERM");
    std::array::from_fn(|i| {
        let word = |bytes: &[u8; MAX_TERM]| {
            u64::from_le_bytes(*bytes[8 * i..].first_chunk().expect("8 of 32 bytes"))
        };
        let word = word(window) & word(mask);
        // Copying each byte's bit 6 into its bit 5 lowercases a letter (A-Z have bit 6 set)
        // and leaves digits and the zero padding alone.
        word | (word >> 1 & 0x2020_2020_2020_2020)
    })
}

/// Multiplicative hash of the exclusive or of a key's words, one multiply rather than one per
/// word. Slots come from its top bits, which depend on every word.
fn key_hash(key: &Key) -> u64 {
    (key.iter().fold(0, |hash, &word| hash ^ word)).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

/// One slot of a [`TermTable`]: a term's first key word, its id and what the corpus pass does
/// with it, 16 bytes that one load brings in.
#[derive(Clone, Copy)]
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

/// `len` empty slots on their own transparent huge pages, never freed. The pass reads the term
/// table at random, and on 4 KiB pages most of those reads would first walk the page tables.
fn huge_slots(len: usize) -> &'static mut [Slot] {
    let layout = Layout::from_size_align(size_of::<Slot>() * len, 2 << 20).expect("fits");
    // SAFETY: `len` is a power of two, so the size is nonzero.
    let block = unsafe { std::alloc::alloc(layout) };
    assert!(!block.is_null(), "out of memory");
    // SAFETY: `block` holds `layout.size()` bytes, aligned for slots, and an all-zero slot is
    // empty; the advice changes no data.
    unsafe {
        madvise(block, layout.size(), MADV_HUGEPAGE);
        block.write_bytes(0, layout.size());
        std::slice::from_raw_parts_mut(block.cast(), len)
    }
}

/// Open-addressing map from query term to its [`Slot`]. A probe compares a slot's `head` with
/// the token's first key word; only a term longer than 8 bytes, rare among tokens, then loads
/// the rest of its key. So most tokens cost one memory access here, whether or not they are
/// query terms. It doubles at half load, so it assumes nothing about how many distinct terms
/// the queries use.
struct TermTable {
    /// The slot count is a power of two.
    slots: &'static mut [Slot],
    /// Term id → its key.
    keys: Vec<Key>,
}

impl TermTable {
    /// An empty table with a slot per term occurrence, rounded up to a power of two: it only
    /// doubles when more than half of `occurrences` are distinct terms.
    fn new(occurrences: usize) -> Self {
        Self {
            slots: huge_slots(occurrences.next_power_of_two()),
            keys: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.keys.len()
    }

    /// The slot of the query term whose key this is, if any, given `at = self.home(key)`.
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
        let at = self.home(key);
        prefetch(&self.slots[at]);
        at
    }

    fn intern(&mut self, key: Key) -> u32 {
        let at = self.position(&key, self.home(&key));
        if self.slots[at].head != 0 {
            return self.slots[at].id;
        }
        self.keys.push(key);
        if self.keys.len() * 2 <= self.slots.len() {
            self.place(self.keys.len() - 1, at);
        } else {
            // Rebuilt from the keys in id order, which reads them sequentially; each new slot
            // is prefetched 16 keys ahead.
            // shortcut: the old table is never freed, which costs memory only when more than
            // half the term occurrences are distinct terms.
            self.slots = huge_slots(self.slots.len() * 2);
            for id in 0..self.keys.len() {
                if let Some(ahead) = self.keys.get(id + 16) {
                    self.prefetch(ahead);
                }
                let key = &self.keys[id];
                self.place(id, self.position(key, self.home(key)));
            }
        }
        self.keys.len() as u32 - 1
    }

    /// Puts term `id` into the free slot `at`.
    fn place(&mut self, id: usize, at: usize) {
        let key = &self.keys[id];
        let flags = if key[1] != 0 { Slot::LONG } else { 0 };
        let (head, id) = (key[0], id as u32);
        self.slots[at] = Slot { head, id, flags };
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
