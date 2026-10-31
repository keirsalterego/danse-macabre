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
//! 2. **Triggers.** Tokenizing the first 1/[`SAMPLE_SHARE`] of the corpus estimates how many
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

// The AVX2 intrinsics of `classify` and the prefetch of `prefetch`.
use std::{
    alloc::Layout, arch::asm, arch::x86_64::*, error::Error, fs::File, io::Write,
    num::NonZeroUsize, ops::Range, os::fd::AsRawFd, path::Path, thread,
};

/// The first `1 / SAMPLE_SHARE` of the corpus is tokenized up front to estimate how rare each
/// query term is.
const SAMPLE_SHARE: usize = 64;
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
    /// libc's `mmap(2)`, used with `PROT_READ` (1) and `MAP_PRIVATE` (2) in [`map_file`]. The
    /// standard library already links libc, so declaring the functions costs no dependency.
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    /// glibc's `mallopt(3)`, used once, with `M_MMAP_THRESHOLD` (-3).
    fn mallopt(param: i32, value: i32) -> i32;
    /// libc's `madvise(2)`, with `MADV_HUGEPAGE` (14) and [`MADV_DONTNEED`].
    fn madvise(addr: *const u8, len: usize, advice: i32) -> i32;
}
const MADV_DONTNEED: i32 = 4;

fn main() -> Result<(), Box<dyn Error>> {
    // glibc raises its mmap threshold to the largest block freed so far, and blocks below it
    // then stay resident after they are freed. Fixed, every block of 128 KiB or more goes back
    // to the kernel when freed, such as each thread's first matches once they are grouped.
    // SAFETY: no other thread exists yet, and mallopt takes no pointers.
    unsafe { mallopt(-3, 128 << 10) }; // M_MMAP_THRESHOLD
    let mut args = std::env::args().skip(1);
    let data_dir = args.next().unwrap_or_else(|| "/data".into());
    let out = Path::new(&args.next().unwrap_or_else(|| "/out".into())).join("results.txt");
    let file = |name| map_file(&Path::new(&data_dir).join(name));
    let parts = answer_all(file("queries.txt")?, file("docs.txt")?)?;
    let mut results = File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    (parts.iter()).try_for_each(|part| results.write_all(part))?;
    // Exit without running destructors: freeing the posting lists would only add to the timed
    // wall. `File` keeps no user-space buffer, so every byte is already written.
    std::process::exit(0)
}

/// Maps the whole file read-only. The page cache is pre-warmed, so this copies nothing and
/// charges almost nothing to our memory cgroup, unlike reading it into the heap.
fn map_file(path: &Path) -> Result<&'static [u8], String> {
    let err = |e| format!("{}: {e}", path.display());
    let file = File::open(path).map_err(err)?;
    let len = file.metadata().map_err(err)?.len() as usize;
    // SAFETY: a new read-only private mapping of an open fd; the kernel picks the address. An
    // empty file maps one byte past its end, never read, as mmap rejects a zero length.
    let ptr = unsafe { mmap(std::ptr::null_mut(), len.max(1), 1, 2, file.as_raw_fd(), 0) };
    if ptr.addr() == usize::MAX {
        return Err(err(std::io::Error::last_os_error()));
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
    let plan = Plan::new(&queries, terms, docs, threads);
    let bounds = line_bounds(docs, threads);
    // Line `i` holds id `i` (spec §2.1), so the id at a chunk's first line numbers all of its
    // lines, and the id after the last line is the number of documents.
    let firsts = (bounds.iter().map(|&at| id_at(docs, at))).collect::<Result<Vec<_>, _>>()?;
    // A query's id sum over a chunk stays below the square of the document count.
    // shortcut: 2^20 or more documents stop here; the spec's full scale has 250,000. Widen the
    // cells to u128 if a corpus that large must run.
    if (firsts.last()).is_some_and(|&n| u64::from(n).pow(2) >> SUM_BITS != 0) {
        return Err("docs.txt: too many documents".into());
    }
    let chunks = parallel(threads, &|c| {
        let (text, ids) = (&docs[bounds[c]..bounds[c + 1]], firsts[c]..firsts[c + 1]);
        scan(text, ids, &queries, &plan)
    });
    let range = |t| queries.len() * t / threads..queries.len() * (t + 1) / threads;
    Ok(parallel(threads, &|t| format_range(range(t), &chunks)))
}

/// Runs `work(i)` for each `i` in `0..n`, each on a thread of its own, and returns the results
/// in order.
fn parallel<T: Send>(n: usize, work: &(impl Fn(usize) -> T + Sync)) -> Vec<T> {
    thread::scope(|s| {
        let threads: Vec<_> = (0..n).map(|i| s.spawn(move || work(i))).collect();
        let join = |t: thread::ScopedJoinHandle<T>| t.join().expect("a panic aborts the process");
        threads.into_iter().map(join).collect()
    })
}

/// Splits `docs` at `parts - 1` line starts, so every line lands in exactly one chunk.
fn line_bounds(docs: &[u8], parts: usize) -> Vec<usize> {
    let mut bounds = vec![0];
    // The last cut, at `docs.len()`, finds no LF after it and ends there.
    for i in 1..=parts {
        let at = docs.len() * i / parts;
        let newline = docs[at..].iter().position(|&b| b == b'\n');
        bounds.push(newline.map_or(docs.len(), |p| at + p + 1));
    }
    bounds
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    And,
    Or,
    Not,
}

/// A query term with the operator that joins it to the terms before it ([`Queries::ops`]).
type Step = (Op, u32);

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

    /// Query `q`'s terms, each with the operator that joins it. The zip ends with the terms.
    fn steps(&self, q: usize) -> impl Iterator<Item = Step> {
        (self.ops[self.at[q] as usize..].iter().copied()).zip(self.terms(q).iter().copied())
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
/// one thread gives each distinct term its id, in file order.
fn parse_queries(text: &[u8], threads: usize) -> Result<(Queries, TermTable), String> {
    let bounds = line_bounds(text, threads);
    let parts = parallel(threads, &|i| parse_lines(&text[bounds[i]..bounds[i + 1]]));
    let parts = parts.into_iter().collect::<Result<Vec<_>, _>>()?;
    let occurrences = parts.iter().map(|part| part.keys.len()).sum();
    let (mut queries, mut table) = (Queries::default(), TermTable::new(occurrences));
    queries.at.push(0);
    for part in parts {
        queries.phrase.extend(part.phrase);
        for len in part.lens {
            queries.at.push(queries.at[queries.at.len() - 1] + len);
        }
        queries.ops.extend(part.ops);
        (queries.terms).extend(part.keys.iter().map(|&key| table.intern(key)));
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
            return Err(format!("queries.txt: not a query: {}", line.escape_ascii()));
        }
        lines.lens.push((lines.keys.len() - terms) as u32);
    }
    Ok(lines)
}

/// The BOOL operators by name (spec §2.3).
const OPS: [(&[u8], Op); 3] = [(b"AND", Op::And), (b"OR", Op::Or), (b"NOT", Op::Not)];

/// Appends the query on `line` to `lines`, or returns `None` if the line is not one.
fn parse_line(line: &[u8], lines: &mut Lines) -> Option<()> {
    // Rejecting anything but `[a-z0-9]{1,32}` keeps a term from matching a token that differs
    // from it only in case, which the case-folding key would otherwise allow, and keeps every
    // term within one `Key`. An empty term's key is all zeros, an empty slot's mark.
    let mut push = |op, term: &[u8]| {
        let lower = term.iter().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9'));
        lines.keys.push(key(term).filter(|k| lower && k[0] != 0)?);
        lines.ops.push(op);
        Some(())
    };
    let phrase = (line.strip_prefix(b"PHRASE \"")).and_then(|r| r.strip_suffix(b"\""));
    if let Some(words) = phrase {
        (words.split(|&b| b == b' ')).try_for_each(|term| push(Op::Or, term))?;
    } else {
        // Terms sit at even positions and operators at odd ones, so a term spelled `and` is an
        // ordinary word and `AND` there is malformed.
        let mut words = line.strip_prefix(b"BOOL ")?.split(|&b| b == b' ');
        push(Op::Or, words.next()?)?;
        while let Some(op) = words.next() {
            let (_, op) = OPS.iter().find(|(name, _)| *name == op)?;
            push(*op, words.next()?)?;
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
    fn new(queries: &Queries, mut terms: TermTable, docs: &[u8], threads: usize) -> Self {
        let (hits, sampled) = sample_hits(docs, &terms, threads);
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
        // How many `Store::Bits` rows and `Store::List` lists are numbered so far.
        let mut sizes = [0; 2];
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
                    // A common term keeps a bitset row, any other an id list: the next of its kind.
                    let kind = usize::from(!common(t));
                    store[t as usize] = [Store::Bits, Store::List][kind](sizes[kind]);
                    sizes[kind] += 1;
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
            rows: sizes[0] as usize,
            lists: sizes[1] as usize,
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
fn bool_cover(steps: impl Iterator<Item = Step>, cost: impl Fn(u32) -> u64, cover: &mut Vec<u32>) {
    cover.clear();
    for (op, term) in steps {
        if op == Op::And && cost(term) < cover.iter().map(|&t| cost(t)).sum() {
            cover.clear();
        } else if op != Op::Or {
            continue;
        }
        cover.push(term);
    }
    cover.sort_unstable();
    cover.dedup();
}

impl Plan {
    /// The phrases that term `t` triggers, as (query index, position of `t` in the phrase).
    fn phrases(&self, t: u32) -> &[(u32, u32)] {
        &self.phrases[self.phrase_at[t as usize] as usize..self.phrase_at[t as usize + 1] as usize]
    }

    /// The indices of the per-document checks that term `t` triggers.
    fn check_range(&self, t: u32) -> Range<usize> {
        self.check_at[t as usize] as usize..self.check_at[t as usize + 1] as usize
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
    let mut grouped = vec![T::default(); items.len()];
    for &(k, item) in items {
        grouped[at[k as usize] as usize] = item;
        at[k as usize] += 1;
    }
    // Each group's start moved to its end, the next group's start.
    at.rotate_right(1);
    at[0] = 0;
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
    fn new(q: u32, steps: impl Iterator<Item = Step>, pad: u32) -> Option<Self> {
        let (mut terms, mut n, mut table) = ([pad; CHECK_TERMS], 0, 0);
        for (op, t) in steps {
            let j = terms[..n].iter().position(|&u| u == t).unwrap_or(n);
            *terms.get_mut(j)? = t;
            n = n.max(j + 1);
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
        for (j, _) in (self.terms.iter().enumerate()).filter(|(_, t)| skip.contains(t)) {
            self.table &= !HAS[j];
        }
        self
    }

    /// Whether a document matches, given `seen`, where bit `t` is set when it holds term `t`.
    /// One AVX2 gather loads the 32-bit words that hold the six terms' bits, and a shift moves
    /// each bit to the top of its lane, where `movemask` collects them as the table's index.
    #[target_feature(enable = "avx2")]
    fn matches(&self, seen: &[u64]) -> bool {
        let lanes = _mm256_setr_epi32(-1, -1, -1, -1, -1, -1, 0, 0);
        // SAFETY: the mask reads the six lanes of `terms` only, and zeroes the other two.
        let terms = unsafe { _mm256_maskload_epi32(self.terms.as_ptr().cast(), lanes) };
        let at = _mm256_srli_epi32::<5>(terms);
        // SAFETY: each lane's word index `t / 32` lies inside `seen`, as `t < 64 * seen.len()`.
        let words = unsafe { _mm256_i32gather_epi32::<4>(seen.as_ptr().cast(), at) };
        let top = _mm256_sllv_epi32(words, _mm256_andnot_si256(terms, _mm256_set1_epi32(31)));
        // The two zeroed lanes hold term 0's bit; the mask drops them.
        self.table >> (_mm256_movemask_ps(_mm256_castsi256_ps(top)) & 63) & 1 != 0
    }
}

/// Counts, for each term, the documents in the first `1 / SAMPLE_SHARE` of the corpus that
/// contain it, and returns those counts with the number of documents sampled: a cheap estimate
/// of document frequency. Each of `threads` threads counts a part of the sample.
fn sample_hits(docs: &[u8], terms: &TermTable, threads: usize) -> (Vec<u32>, usize) {
    let sample = &docs[..line_bounds(docs, SAMPLE_SHARE)[1]];
    let bounds = line_bounds(sample, threads);
    thread::scope(|s| {
        let counters: Vec<_> = (bounds.windows(2))
            .map(|w| s.spawn(move || count_hits(&sample[w[0]..w[1]], terms)))
            .collect();
        // The sum starts from the first part, whose pages are written already: a new zeroed
        // vector would fault twice on each page, once for the load and again for the store.
        (counters.into_iter())
            .map(|c| c.join().expect("a panic aborts the process"))
            .reduce(|(mut hits, docs), (part, more)| {
                hits.iter_mut().zip(part).for_each(|(h, p)| *h += p);
                (hits, docs + more)
            })
            .expect("line_bounds returns at least one part")
    })
}

/// [`sample_hits`] on one part of the sample.
fn count_hits(sample: &[u8], terms: &TermTable) -> (Vec<u32>, usize) {
    // Written when allocated, as in [`Tally::new`], so that no page faults twice.
    let mut hits = [0].repeat(terms.len());
    // Per term, the last sampled document counted, so a document counts once.
    let mut counted = vec![usize::MAX; terms.len()];
    let mut doc = 0;
    let token = |&mut doc: &mut usize, start, end| {
        if let Some(key) = key_in(sample, start, end)
            // A token that is no query term counts at id 0, which no query reads.
            && let slot = terms.find_from(&key, terms.home(&key))
            && counted[slot.id() as usize] != doc
        {
            counted[slot.id() as usize] = doc;
            hits[slot.id() as usize] += 1;
        }
    };
    walk(sample, &mut doc, token, |doc| *doc += 1);
    (hits, doc)
}

/// The id of the line that starts at `at`, or the number of documents when `at` is the end of
/// `docs`: the last line's id plus one. Line `i` holds id `i` (spec §2.1), so the ids of the
/// lines after it follow by counting. An empty `docs.txt` stops here with an error: its lines
/// end in LF (spec §2.1), so it holds at least one.
fn id_at(docs: &[u8], at: usize) -> Result<u32, String> {
    if at == docs.len() && at > 0 {
        let lf = docs[..at - 1].iter().rposition(|&b| b == b'\n');
        return Ok(id_at(docs, lf.map_or(0, |lf| lf + 1))? + 1);
    }
    let digits = docs[at..].iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || docs.get(at + digits) != Some(&b'\t') {
        return Err(format!("docs.txt: no id and TAB at byte {at}"));
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
    // 1 when the byte before the block is a token byte; the start of a token that runs on past
    // the block; where the current line starts.
    let (mut prev_bit, mut open, mut line_start) = (0, None, 0);
    // The last block, short of 64 bytes, padded with zero bytes, which are separators.
    let (mut padded, tail) = ([0; 64], chunk.as_chunks::<64>().1);
    padded[..tail.len()].copy_from_slice(tail);
    for base in (0..chunk.len()).step_by(64) {
        let block = chunk[base..].first_chunk().unwrap_or(&padded);
        // SAFETY: the binary targets alderlake (.cargo/config.toml), which always has AVX2.
        let (token, lf) = unsafe { classify(block) };
        let prev = token << 1 | prev_bit;
        let (mut starts, mut ends, mut lfs) = (token & !prev, !token & prev, lf);
        prev_bit = token >> 63;
        while ends != 0 {
            let end = base + pop(&mut ends);
            let start = open.take().unwrap_or_else(|| base + pop(&mut starts));
            while lfs != 0 && base + (lfs.trailing_zeros() as usize) < start {
                line_start = base + pop(&mut lfs) + 1;
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
            line_start = base + pop(&mut lfs) + 1;
            on_line_end(state);
        }
    }
}

/// Clears the lowest set bit of `bits`, which is not 0, and returns its position.
fn pop(bits: &mut u64) -> usize {
    std::mem::replace(bits, *bits & (*bits - 1)).trailing_zeros() as usize
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
        // Written here, each page of cells faults in once, as a page of its own. Zeroed by
        // calloc, as `vec!` does, the pass's first load from a page maps the shared zero page,
        // and the store after that load replaces it, which interrupts every core to flush its
        // TLB.
        Self {
            queries: [0].repeat(queries),
            checks: [0].repeat(checks),
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
            self.add(q, base + pop(&mut word) as u32);
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
    /// `ids[at[q]..at[q + 1]]`.
    at: Vec<u32>,
    ids: Vec<u32>,
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
    // Bit `t` is set while the current doc contains term `t`, bit 0 while it holds a token that
    // is no query term. One more bit, never set, pads the checks ([`Check`]).
    let mut seen = vec![0u64; (plan.terms.len() + 1).div_ceil(64)];
    // The current doc's term id per token (0 for the rest), the positions of its phrase
    // triggers, the checks of its distinct BOOL triggers, and the phrases it matched, so none
    // counts twice.
    let (mut terms, mut phrase_at, mut bool_terms) = (Vec::new(), Vec::new(), Vec::new());
    // Its distinct stored terms and BOOL triggers, in the first `stored` and `bools` entries:
    // one entry per token, so that keeping a term takes a store and an add, not a branch.
    let (mut kept, mut triggered) = (Vec::new(), Vec::new());
    let mut found = Vec::new();
    // The current doc's tokens as (key, home slot), probed only at the line end: by then every
    // slot load is long under way ([`TermTable::prefetch`]), instead of each stalling in turn.
    let mut probes: Vec<(Key, usize)> = Vec::new();
    let mut doc = ids.start;
    let token = |probes: &mut Vec<_>, start, end| {
        // A token past `MAX_TERM` bytes takes a key of 0xFF bytes, which no term has.
        let key = key_in(text, start, end).unwrap_or([!0; MAX_TERM / 8]);
        probes.push((key, plan.terms.prefetch(&key)));
    };
    walk(text, &mut probes, token, |probes| {
        if kept.len() < probes.len() {
            kept.resize(probes.len(), 0);
            triggered.resize(probes.len(), 0);
        }
        let (mut stored, mut bools) = (0, 0);
        // No branch on what a token is, a query term or not, seen in the doc before or not,
        // stored or a BOOL trigger: none of it follows a pattern, so each branch would
        // mispredict often.
        for (key, at) in probes.drain(..) {
            let slot = plan.terms.find_from(&key, at);
            let (t, bit) = (slot.id() as usize / 64, 1 << (slot.id() % 64));
            // The term's flags if the doc did not hold it yet, else none: a term is kept and
            // triggers once per doc.
            let new = std::hint::select_unpredictable(seen[t] & bit == 0, slot.flags, 0);
            seen[t] |= bit;
            kept[stored] = slot.id();
            stored += usize::from(new & Slot::STORED != 0);
            triggered[bools] = slot.id();
            bools += usize::from(new & Slot::BOOL != 0);
            if slot.flags & Slot::PHRASE != 0 {
                phrase_at.push(terms.len());
            }
            terms.push(slot.id());
        }
        for &t in &triggered[..bools] {
            // Start loading its checks and their cells now, to use below.
            let checks = plan.check_range(t);
            prefetch(&plan.checks[checks.start]);
            prefetch(&tally.checks[checks.start]);
            // Four indices written at once, then cut to the range's length: a loop
            // per trigger would mispredict its exit, as the lengths follow no pattern.
            let start = checks.start as u32;
            bool_terms.extend_from_slice(&[start, start + 1, start + 2, start + 3]);
            bool_terms.truncate(bool_terms.len() - 4 + checks.len().min(4));
            bool_terms.extend(checks.skip(4).map(|c| c as u32));
        }
        for &t in &kept[..stored] {
            sets.add(plan.store[t as usize], doc);
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
        for c in bool_terms.drain(..) {
            let check = &plan.checks[c as usize];
            // SAFETY: the binary targets alderlake (.cargo/config.toml), which has AVX2.
            tally.add_check(c as usize, check.q, doc, unsafe { check.matches(&seen) });
        }
        // Every set bit is one of the doc's terms, so zeroing their words clears `seen`: a store
        // with no load, so tokens that share a word, as all misses do, do not wait on each other.
        for &t in &terms {
            // SAFETY: `terms` holds term ids and 0, all below `64 * seen.len()`.
            *unsafe { seen.get_unchecked_mut(t as usize / 64) } = 0;
        }
        terms.clear();
        phrase_at.clear();
        found.clear();
        doc += 1;
        if doc - sets.start == 64 * WINDOW as u32 {
            sets.flush(queries, plan, &mut tally);
        }
    });
    // The chunk's pages leave our page tables now, each thread its own, instead of one by one
    // in the exit after the output is written.
    let offset = text.as_ptr().addr() % 4096;
    let start = text.as_ptr().wrapping_sub(offset);
    // SAFETY: madvise only drops pages. Those of the docs mapping, private and never written,
    // fault the same bytes back in if read again, as a neighbouring chunk's first or last page
    // may be; outside a mapping, madvise fails and changes nothing.
    unsafe { madvise(start, offset + text.len(), MADV_DONTNEED) };
    sets.flush(queries, plan, &mut tally);
    // The window's bitsets go back to the kernel before the first matches are grouped, when
    // memory peaks.
    drop(sets);
    // The checks' cells hold their queries' whole counts and sums.
    for (check, cell) in plan.checks.iter().zip(std::mem::take(&mut tally.checks)) {
        tally.queries[check.q as usize] += cell;
    }
    let (at, ids) = tally.take_first();
    Chunk { tally, at, ids }
}

/// Formats the answers to queries `range`, in order (spec §4).
fn format_range(range: Range<usize>, chunks: &[Chunk]) -> Vec<u8> {
    let mut out = Vec::new();
    for q in range {
        // The id sums of all chunks together stay below `2^SUM_BITS`, as each one does
        // ([`answer_all`]), so the cells add up without a carry into the count.
        let cell: u64 = chunks.iter().map(|c| c.tally.queries[q]).sum();
        let (count, sum) = (cell >> SUM_BITS, cell & ((1 << SUM_BITS) - 1));
        write!(out, "{count} {sum}").expect("writing to a Vec cannot fail");
        // Chunks hold ascending ids, so the first chunks' first matches are the smallest.
        let shown = (chunks.iter()).flat_map(|c| &c.ids[c.at[q] as usize..c.at[q + 1] as usize]);
        for id in shown.take(SHOWN_IDS) {
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
    let weight = |(k, &mask): (usize, &u64)| u64::from((word & mask).count_ones()) << k;
    HAS.iter().enumerate().map(weight).sum()
}

/// Starts loading the cache line that holds `item` into L1.
fn prefetch<T>(item: &T) {
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
    // Flattened, `ONES` is 32 bytes of 0xFF then 32 zeros, so `len` bytes of 0xFF, then zeros,
    // start at `MAX_TERM - len`. On one 64-byte cache line, that 32-byte load never splits
    // across two; unaligned, the table sat wherever the rest of the constants left it, and a
    // split load per token cost up to 1% of the run.
    #[repr(align(64))]
    struct Line([[u8; MAX_TERM]; 2]);
    const ONES: Line = Line([[0xFF; MAX_TERM], [0; MAX_TERM]]);
    let mask: &[u8; MAX_TERM] = ONES.0.as_flattened()[MAX_TERM - len..]
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

/// One slot of a [`TermTable`]: a term's first 12 bytes, its id and what the corpus pass does
/// with it, 16 bytes that one load brings in.
#[derive(Clone, Copy)]
struct Slot {
    /// `key[0]`, the term's first 8 bytes; 0 marks an empty slot, as a term's first byte is
    /// never zero.
    head: u64,
    /// The low half of `key[1]`, the term's bytes 8 to 11.
    tail: u32,
    /// The term id times 16, plus `LONG` and the [`Plan`]'s flags for the term.
    flags: u32,
}

impl Slot {
    /// The term runs past 12 bytes, so `head` and `tail` alone do not identify it.
    const LONG: u32 = 1;
    /// The term triggers phrase checks.
    const PHRASE: u32 = 2;
    /// The term triggers per-document BOOL checks.
    const BOOL: u32 = 4;
    /// The corpus pass keeps the term's documents ([`Store`]).
    const STORED: u32 = 8;

    /// The term id.
    fn id(self) -> u32 {
        self.flags >> 4
    }
}

/// Open-addressing map from query term to its [`Slot`]. A probe compares a slot's `head` and
/// `tail` with the token's first 12 bytes; only a term longer than 12 bytes, rare among tokens,
/// then loads the rest of its key. So most tokens cost one memory access here, whether or not
/// they are query terms. It holds 3 slots per 2 term occurrences or more, so it is at most two
/// thirds full whatever share of the occurrences are distinct terms.
struct TermTable {
    /// The slot count is a power of two.
    slots: &'static mut [Slot],
    /// Term id → its key. Id 0 is no term: it is the id of an empty slot, all zero, which a
    /// probe for a token that is no query term returns.
    keys: Vec<Key>,
}

impl TermTable {
    /// An empty table for the terms of `occurrences` term occurrences: 3 slots per 2, plus one,
    /// rounded up to a power of two. A probe for a key that is no term stops at an empty slot,
    /// and there is always one. For the spec's 150,000 or so occurrences, of which about half
    /// are distinct terms, that is 262,144 slots, at most 29% full.
    ///
    /// The slots sit alone on transparent huge pages and are never freed. The pass reads them
    /// at random, and on 4 KiB pages most of those reads would first walk the page tables.
    fn new(occurrences: usize) -> Self {
        // A term id times 16 fits in `Slot::flags`.
        assert!(occurrences < 1 << 28, "queries.txt: too many terms");
        let len = (occurrences * 3 / 2 + 1).next_power_of_two();
        let layout = Layout::from_size_align(size_of::<Slot>() * len, 2 << 20).expect("fits");
        // SAFETY: `len` is at least 1, so the size is nonzero.
        let block = unsafe { std::alloc::alloc(layout) };
        assert!(!block.is_null(), "out of memory");
        // SAFETY: `block` holds `layout.size()` bytes, aligned for slots, and an all-zero slot
        // is empty; the advice changes no data.
        let slots = unsafe {
            madvise(block, layout.size(), 14); // MADV_HUGEPAGE
            block.write_bytes(0, layout.size());
            std::slice::from_raw_parts_mut(block.cast(), len)
        };
        let keys = vec![[0; MAX_TERM / 8]];
        Self { slots, keys }
    }

    fn len(&self) -> usize {
        self.keys.len()
    }

    /// The slot of the query term whose key this is, given `at = self.home(key)`, or else the
    /// empty slot where its probe ends, whose id, 0, is no term's.
    fn find_from(&self, key: &Key, at: usize) -> Slot {
        self.slots[self.position(key, at)]
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

    /// The id of the term whose key this is. A new term takes the next id and the free slot
    /// where its probe ends.
    fn intern(&mut self, key: Key) -> u32 {
        let at = self.position(&key, self.home(&key));
        if self.slots[at].head == 0 {
            let long = u32::from(key[1] >> 32 | key[2] | key[3] != 0) * Slot::LONG;
            let (head, tail, flags) = (key[0], key[1] as u32, (self.keys.len() as u32) << 4 | long);
            self.slots[at] = Slot { head, tail, flags };
            self.keys.push(key);
        }
        self.slots[at].id()
    }

    /// Adds `flags[id]` to the flags of each term `id`.
    fn mark(&mut self, flags: &[u32]) {
        for slot in self.slots.iter_mut().filter(|slot| slot.head != 0) {
            slot.flags |= flags[slot.id() as usize];
        }
    }

    /// The slot holding `key`, or the empty slot where it belongs, probing linearly from `at`.
    fn position(&self, key: &Key, mut at: usize) -> usize {
        let mask = self.slots.len() - 1;
        let long = key[1] >> 32 | key[2] | key[3] != 0;
        loop {
            let slot = &self.slots[at];
            // No `&&`: whether a token is a query term follows no pattern, so the probe takes
            // one branch, mostly taken, which ends it at the term or at an empty slot.
            let mut found = (slot.head ^ key[0]) | u64::from(slot.tail ^ key[1] as u32) == 0;
            found &= (slot.flags & Slot::LONG != 0) == long;
            if found & long {
                found = self.keys[slot.id() as usize] == *key;
            }
            // Hidden from the optimizer, the test stays one branch: LLVM would split it in two,
            // the first of which mispredicts on every token that is no query term.
            let mut stop = u8::from(slot.head == 0) | u8::from(found);
            // SAFETY: the asm block is empty; it only keeps the optimizer from seeing `stop`.
            unsafe { asm!("/* {} */", inout(reg_byte) stop, options(pure, nomem)) };
            if stop != 0 {
                return at;
            }
            at = (at + 1) & mask;
        }
    }
}
