//! Track B: answer BOOL and PHRASE queries over a corpus of one document per line.
//!
//! Pipeline:
//! 1. Read `queries.txt` first (spec §3 allows it) and intern every distinct query term. Corpus
//!    tokens that no query uses are looked up once and then forgotten.
//! 2. Tokenize the first `SAMPLE_DOCS` documents and count each term's hits. A phrase's trigger
//!    is its rarest term in that sample. The choice only affects speed: every match contains it.
//! 3. One parallel pass over `docs.txt`, one chunk of whole lines per thread. [`walk`] finds
//!    tokens 64 bytes at a time from AVX2 bit masks, never byte by byte. A BOOL term appends
//!    the doc id to the thread's posting list for it. At the end of each doc, every trigger
//!    occurrence checks its phrases' windows against the doc's token ids, so no document is
//!    ever tokenized twice.
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

/// What the corpus pass needs to know about each query term.
struct Plan<'q> {
    terms: TermTable<'q>,
    /// Per term: does a BOOL query use it, so the pass must keep its doc ids?
    in_bool: Vec<bool>,
    /// Per term: (query index, position in the phrase) of every phrase it triggers.
    triggers: Vec<Vec<(u32, u32)>>,
}

impl<'q> Plan<'q> {
    fn new(queries: &[Query], terms: TermTable<'q>, docs: &[u8]) -> Self {
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
        let hits = sample_hits(docs, &terms);
        let mut triggers = vec![Vec::new(); terms.len()];
        for (i, query) in queries.iter().enumerate().filter(|(_, q)| q.phrase) {
            // Ties (often zero hits) go to the longer term, which tends to be rarer.
            let (offset, &term) = (query.terms.iter().enumerate())
                .min_by_key(|&(_, &t)| (hits[t as usize], Reverse(terms.name(t).len())))
                .expect("the parser rejects a phrase without terms");
            triggers[term as usize].push((i as u32, offset as u32));
        }
        Self {
            terms,
            in_bool,
            triggers,
        }
    }
}

/// Hits per term in the first `SAMPLE_DOCS` documents: a cheap stand-in for document frequency.
fn sample_hits(docs: &[u8], terms: &TermTable) -> Vec<u32> {
    let mut lines = docs.iter().enumerate().filter(|&(_, &b)| b == b'\n');
    let end = lines
        .nth(SAMPLE_DOCS - 1)
        .map_or(docs.len(), |(at, _)| at + 1);
    let sample = &docs[..end];
    let mut hits = vec![0; terms.len()];
    walk(sample, |event| {
        if let Event::Token { start, end } = event
            && let Some(t) = key_in(sample, start, end).and_then(|key| terms.find(&key))
        {
            hits[t as usize] += 1;
        }
    });
    hits
}

/// The id on the first line of `chunk`. Line `i` of `docs.txt` holds id `i` (spec §2.1), so
/// the ids of the lines after it follow by counting LFs.
fn first_id(chunk: &[u8]) -> Result<u32, String> {
    let digits = chunk.iter().take_while(|b| b.is_ascii_digit()).count();
    if !chunk.is_empty() && (digits == 0 || chunk.get(digits) != Some(&b'\t')) {
        let start = String::from_utf8_lossy(&chunk[..chunk.len().min(40)]);
        return Err(format!(
            "docs.txt: no id and TAB at the line starting {start:?}"
        ));
    }
    Ok((chunk[..digits].iter()).fold(0, |id, &digit| id * 10 + u32::from(digit - b'0')))
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

/// What one thread keeps from its chunk of whole lines, which covers one contiguous, ascending
/// range of doc ids. Chunks stay separate: BOOL works chunk by chunk, so nothing is concatenated.
struct Segment {
    /// Per BOOL term, the ascending ids of this chunk's documents that contain it.
    postings: Vec<Vec<u32>>,
    /// (query index, doc id) of every phrase match, repeated if a phrase matches twice in a doc.
    matches: Vec<(u32, u32)>,
}

/// Runs the corpus pass over one chunk of whole lines: posting lists for BOOL terms and the
/// phrase matches, each phrase checked at the end of its document.
fn scan(chunk: &[u8], queries: &[Query], plan: &Plan) -> Result<Segment, String> {
    let mut postings = vec![Vec::new(); plan.in_bool.len()];
    let mut matches = Vec::new();
    // The current doc's term id per token (MISS for the rest), and where its triggers sit.
    let mut ids = Vec::new();
    let mut triggered = Vec::new();
    let mut doc = first_id(chunk)?;
    walk(chunk, |event| match event {
        Event::Token { start, end } => {
            let Some(term) = key_in(chunk, start, end).and_then(|key| plan.terms.find(&key)) else {
                ids.push(MISS);
                return;
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
        Event::LineEnd => {
            for &at in &triggered {
                for &(query, offset) in &plan.triggers[ids[at] as usize] {
                    let phrase: &[u32] = &queries[query as usize].terms;
                    // `ids` holds this doc only, so a window never spans two docs (spec §3).
                    let start = at.checked_sub(offset as usize);
                    if start.and_then(|s| ids.get(s..s + phrase.len())) == Some(phrase) {
                        matches.push((query, doc));
                    }
                }
            }
            ids.clear();
            triggered.clear();
            doc += 1;
        }
    });
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

/// Open-addressing map from query term to dense id. A probe reads a slot's id and compares
/// that term's key, one fixed-size compare with no loop over bytes. Keys sit densely by id, so
/// the table stays a few MB. It doubles at half load, so it assumes nothing about how many
/// distinct terms the queries use.
struct TermTable<'q> {
    /// Id + 1 per slot; 0 marks an empty slot. The slot count is a power of two.
    slots: Vec<u32>,
    /// Term id → its key.
    keys: Vec<Key>,
    /// Term id → the term as written in `queries.txt`.
    names: Vec<&'q [u8]>,
}

impl Default for TermTable<'_> {
    fn default() -> Self {
        Self {
            slots: vec![0; 1024],
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

    /// The id of the query term whose key this is, if any.
    fn find(&self, key: &Key) -> Option<u32> {
        self.slots[self.slot(key)].checked_sub(1)
    }

    fn intern(&mut self, name: &'q [u8]) -> u32 {
        let key = key(name).expect("the parser rejects terms over MAX_TERM bytes");
        let slot = self.slot(&key);
        if let Some(id) = self.slots[slot].checked_sub(1) {
            return id;
        }
        self.keys.push(key);
        self.names.push(name);
        self.slots[slot] = self.names.len() as u32;
        if self.names.len() * 2 > self.slots.len() {
            let grown = vec![0; self.slots.len() * 2];
            let old = std::mem::replace(&mut self.slots, grown);
            for id in old.into_iter().filter(|&id| id != 0) {
                let slot = self.slot(&self.keys[id as usize - 1]);
                self.slots[slot] = id;
            }
        }
        self.names.len() as u32 - 1
    }

    /// The slot holding `key`, or the empty slot where it belongs (linear probing).
    fn slot(&self, key: &Key) -> usize {
        let mask = self.slots.len() - 1;
        let mut slot = (key_hash(key) >> (64 - self.slots.len().trailing_zeros())) as usize;
        while let Some(id) = self.slots[slot].checked_sub(1) {
            if self.keys[id as usize] == *key {
                break;
            }
            slot = (slot + 1) & mask;
        }
        slot
    }
}
