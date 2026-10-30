//! Track B: answer BOOL and PHRASE queries over a corpus of one document per line.
//!
//! Pipeline:
//! 1. Read `queries.txt` first (spec §3 allows it) and intern every distinct query term. Corpus
//!    tokens that no query uses are looked up once and then forgotten.
//! 2. Tokenize the first `SAMPLE_DOCS` documents and count each term's hits. A phrase's trigger
//!    is its rarest term in that sample. The choice only affects speed: every match contains it.
//! 3. One parallel pass over `docs.txt`, one chunk of whole lines per thread. A BOOL term
//!    appends the doc id to the thread's posting list for it. At the end of each doc, every
//!    trigger occurrence checks its phrases' windows against the doc's token ids, so no document
//!    is ever tokenized twice.
//! 4. Threads answer contiguous ranges of queries. BOOL merges the sorted lists strictly left
//!    to right, chunk by chunk; PHRASE reads the matches the pass recorded.
//!
//! The corpus is read from the mmap and never copied. Numbers: PERF.md.

use std::cmp::{Ordering, Reverse};
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

/// Byte → its token character after ASCII lowercasing, or 0 for a separator (spec §2.2). Every
/// byte from 0x80 up is a separator, even inside a UTF-8 letter.
const FOLD: [u8; 256] = {
    let mut table = [0; 256];
    let mut b = 0;
    while b < 256 {
        let c = (b as u8).to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            table[b] = c;
        }
        b += 1;
    }
    table
};

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
    let (queries, terms) = parse_queries(query_text)?;
    let plan = Plan::new(&queries, terms, docs)?;
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let bounds = line_bounds(docs, threads);
    let (queries, plan) = (&queries, &plan);
    let segments = thread::scope(|s| {
        let scanners: Vec<_> = (bounds.windows(2))
            .map(|w| s.spawn(move || scan(&docs[w[0]..w[1]], queries, plan)))
            .collect();
        (scanners.into_iter())
            .map(|h| h.join().expect("a panic aborts the process"))
            .collect::<Result<Vec<_>, _>>()
    })?;
    // Sorting by (query, doc) groups each phrase's matches in ascending doc order; dedup drops
    // the repeats of a phrase that matches twice in one doc.
    let mut hits: Vec<(u32, u32)> = (segments.iter())
        .flat_map(|s| s.matches.iter().copied())
        .collect();
    hits.sort_unstable();
    hits.dedup();
    let (segments, hits, n) = (&segments, &hits, queries.len());
    Ok(thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|t| n * t / threads..n * (t + 1) / threads)
            .map(|range| s.spawn(move || answer_range(range, queries, segments, hits)))
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
        // Rejecting anything but `[a-z0-9]+` keeps a term from matching a token that differs
        // from it only in case, which the case-folding lookup below would otherwise allow.
        let mut term = |w: &'q [u8]| {
            let valid = !w.is_empty()
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

/// What the corpus pass needs to know about each query term.
struct Plan<'q> {
    terms: TermTable<'q>,
    /// Per term: does a BOOL query use it, so the pass must keep its doc ids?
    in_bool: Vec<bool>,
    /// Per term: (query index, position in the phrase) of every phrase it triggers.
    triggers: Vec<Vec<(u32, u32)>>,
}

impl<'q> Plan<'q> {
    fn new(queries: &[Query], terms: TermTable<'q>, docs: &[u8]) -> Result<Self, String> {
        let mut in_bool = vec![false; terms.len()];
        for query in queries.iter().filter(|q| !q.phrase) {
            for &t in &query.terms {
                in_bool[t as usize] = true;
            }
        }
        // Every match of a phrase holds each of its terms at a fixed offset from the window
        // start, so checking the windows around one term's occurrences finds all of them.
        // Checking around the rarest term costs the fewest windows. A term absent from the
        // corpus never fires, which leaves the phrase at `0 0` with no special case.
        let hits = sample_hits(docs, &terms)?;
        let mut triggers = vec![Vec::new(); terms.len()];
        for (i, query) in queries.iter().enumerate().filter(|(_, q)| q.phrase) {
            // Ties (often zero hits) go to the longer term, which tends to be rarer.
            let (offset, &term) = (query.terms.iter().enumerate())
                .min_by_key(|&(_, &t)| (hits[t as usize], Reverse(terms.name(t).len())))
                .expect("the parser rejects a phrase without terms");
            triggers[term as usize].push((i as u32, offset as u32));
        }
        Ok(Self {
            terms,
            in_bool,
            triggers,
        })
    }
}

/// Hits per term in the first `SAMPLE_DOCS` documents: a cheap stand-in for document frequency.
fn sample_hits(docs: &[u8], terms: &TermTable) -> Result<Vec<u32>, String> {
    let mut hits = vec![0; terms.len()];
    for line in docs.split_inclusive(|&b| b == b'\n').take(SAMPLE_DOCS) {
        for token in tokens(split_doc(line)?.1) {
            if let Some(t) = terms.find(token) {
                hits[t as usize] += 1;
            }
        }
    }
    Ok(hits)
}

/// Splits a line of `docs.txt` into its id and its text (spec §2.1). The text keeps the LF,
/// which the tokenizer treats as one more separator.
fn split_doc(line: &[u8]) -> Result<(u32, &[u8]), String> {
    let tab = (line.iter().position(|&b| b == b'\t')).ok_or_else(|| {
        let start = String::from_utf8_lossy(&line[..line.len().min(40)]);
        format!("docs.txt: no TAB in the line starting {start:?}")
    })?;
    let id = (line[..tab].iter()).fold(0, |id, &digit| id * 10 + u32::from(digit - b'0'));
    Ok((id, &line[tab + 1..]))
}

/// The tokens of a document's text, as slices of the raw (not yet lowercased) bytes (spec §2.2).
fn tokens(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    (text.split(|&b| FOLD[b as usize] == 0)).filter(|token| !token.is_empty())
}

/// What one thread keeps from its chunk of whole lines, which covers one contiguous, ascending
/// range of doc ids. Chunks stay separate: BOOL works chunk by chunk, so nothing is concatenated.
struct Segment {
    /// Per BOOL term, the ascending ids of this chunk's documents that contain it.
    postings: Vec<Vec<u32>>,
    /// (query index, doc id) of every phrase match, repeated if a phrase matches twice in a doc.
    matches: Vec<(u32, u32)>,
}

fn scan(chunk: &[u8], queries: &[Query], plan: &Plan) -> Result<Segment, String> {
    let mut postings = vec![Vec::new(); plan.in_bool.len()];
    let mut matches = Vec::new();
    // The current doc's term id per token (MISS for the rest), and where its triggers sit.
    let mut ids = Vec::new();
    let mut triggered = Vec::new();
    for line in chunk.split_inclusive(|&b| b == b'\n') {
        let (doc, text) = split_doc(line)?;
        ids.clear();
        triggered.clear();
        for token in tokens(text) {
            let Some(term) = plan.terms.find(token) else {
                ids.push(MISS);
                continue;
            };
            let t = term as usize;
            // Docs arrive in id order, so a term seen earlier in this doc is the list's tail.
            if plan.in_bool[t] && postings[t].last() != Some(&doc) {
                postings[t].push(doc);
            }
            if !plan.triggers[t].is_empty() {
                triggered.push(ids.len());
            }
            ids.push(term);
        }
        for &at in &triggered {
            for &(query, offset) in &plan.triggers[ids[at] as usize] {
                let phrase: &[u32] = &queries[query as usize].terms;
                // `ids` holds this doc only, so a window can never span two docs (spec §3).
                let start = at.checked_sub(offset as usize);
                if start.and_then(|s| ids.get(s..s + phrase.len())) == Some(phrase) {
                    matches.push((query, doc));
                }
            }
        }
    }
    Ok(Segment { postings, matches })
}

/// Formats the answers to `queries[range]`, in order.
fn answer_range(
    range: Range<usize>,
    queries: &[Query],
    segments: &[Segment],
    hits: &[(u32, u32)],
) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut result, mut acc, mut tmp) = (Vec::new(), Vec::new(), Vec::new());
    for i in range {
        let query = &queries[i];
        result.clear();
        if query.phrase {
            let q = i as u32;
            let lo = hits.partition_point(|&(h, _)| h < q);
            let hi = hits.partition_point(|&(h, _)| h <= q);
            result.extend(hits[lo..hi].iter().map(|&(_, doc)| doc));
        } else {
            // Each operator distributes over the disjoint id ranges of the segments, so
            // evaluating segment by segment in doc order gives the whole result, ascending.
            for seg in segments {
                acc.clear();
                acc.extend_from_slice(&seg.postings[query.terms[0] as usize]);
                for (&op, &term) in query.ops.iter().zip(&query.terms[1..]) {
                    combine(&acc, &seg.postings[term as usize], op, &mut tmp);
                    std::mem::swap(&mut acc, &mut tmp);
                }
                result.extend_from_slice(&acc);
            }
        }
        write_answer(&mut out, &result);
    }
    out
}

/// Writes `a op b` to `out` for two ascending id lists, in one merge pass. `NOT` is set
/// difference (spec §3).
fn combine(a: &[u32], b: &[u32], op: Op, out: &mut Vec<u32>) {
    out.clear();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Less => {
                if op != Op::And {
                    out.push(a[i]);
                }
                i += 1;
            }
            Ordering::Greater => {
                if op == Op::Or {
                    out.push(b[j]);
                }
                j += 1;
            }
            Ordering::Equal => {
                if op != Op::Not {
                    out.push(a[i]);
                }
                i += 1;
                j += 1;
            }
        }
    }
    if op != Op::And {
        out.extend_from_slice(&a[i..]);
    }
    if op == Op::Or {
        out.extend_from_slice(&b[j..]);
    }
}

/// One output line: count, sum of ids, then the smallest ids ascending (spec §4).
fn write_answer(out: &mut Vec<u8>, ids: &[u32]) {
    let sum: u64 = ids.iter().map(|&id| u64::from(id)).sum();
    write!(out, "{} {sum}", ids.len()).expect("writing to a Vec cannot fail");
    for id in ids.iter().take(SHOWN_IDS) {
        write!(out, " {id}").expect("writing to a Vec cannot fail");
    }
    out.push(b'\n');
}

/// Hash of a token after ASCII lowercasing, so `Bail` and `bail` probe the same slot.
fn term_hash(token: &[u8]) -> u64 {
    (token.iter()).fold(0, |hash, &b| {
        (hash.rotate_left(5) ^ u64::from(FOLD[b as usize])).wrapping_mul(0x517c_c1b7_2722_0a95)
    })
}

/// Open-addressing map from query term to dense id. It doubles at half load, so it assumes
/// nothing about how many distinct terms the queries use.
struct TermTable<'q> {
    /// Id + 1 per slot; 0 marks an empty slot.
    slots: Vec<u32>,
    names: Vec<(&'q [u8], u64)>,
}

impl Default for TermTable<'_> {
    fn default() -> Self {
        Self {
            slots: vec![0; 1024],
            names: Vec::new(),
        }
    }
}

impl<'q> TermTable<'q> {
    fn len(&self) -> usize {
        self.names.len()
    }

    fn name(&self, id: u32) -> &'q [u8] {
        self.names[id as usize].0
    }

    /// The id of the query term that a corpus token spells, if any.
    fn find(&self, token: &[u8]) -> Option<u32> {
        self.slots[self.slot(token, term_hash(token))].checked_sub(1)
    }

    fn intern(&mut self, name: &'q [u8]) -> u32 {
        let hash = term_hash(name);
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

    /// The slot holding `name`, or the empty slot where it belongs (linear probing). Names are
    /// all `[a-z0-9]`, so comparing without case is exactly the spec's lowercasing of a token.
    fn slot(&self, name: &[u8], hash: u64) -> usize {
        let mask = self.slots.len() - 1;
        let mut slot = (hash >> 32) as usize & mask;
        while let Some(id) = self.slots[slot].checked_sub(1) {
            let (known, known_hash) = self.names[id as usize];
            if known_hash == hash && known.eq_ignore_ascii_case(name) {
                break;
            }
            slot = (slot + 1) & mask;
        }
        slot
    }
}
