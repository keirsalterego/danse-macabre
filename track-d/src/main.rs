//! # Track D: Traverse
//!
//! For each query `(s, t)` this program prints the length of the shortest directed path from `s`
//! to `t` in a weighted road graph, or `-1` when there is none. At full scale the graph has 1.7M
//! nodes and 5M edges, and there are 2,000 queries. The clock covers everything: reading the
//! files, building the graph, the searches and writing the answers.
//!
//! ## Input
//!
//! `graph.txt` is a header `n m` followed by `m` lines `u v w`, each a directed edge from `u` to
//! `v` with weight `w` in `0..2^32`. The lines are shuffled. Parallel edges, duplicate lines,
//! self-loops and weight-0 edges all occur. `queries.txt` holds one `s t` per line. Both files are
//! mapped read-only with `mmap` ([`map_file`]), and one cursor walks the bytes ([`number`]), so
//! no line is ever copied into a string.
//!
//! ## The graph: two CSR arrays
//!
//! A [`Csr`] (compressed sparse row) stores every edge in one array, grouped by node, next to an
//! offsets array that says where each node's group starts. An edge is one `u64`, the weight in
//! the high half and the neighbour in the low half, so a node's edges are one contiguous slice.
//! There are two of them: `forward` groups the edges by tail for the search from `s`, and
//! `reverse` groups them by head for the search from `t`.
//!
//! [`build_graph`] fills both in two parallel passes over the mapped text, one thread per chunk
//! of lines. The first pass counts each node's degree with atomic adds. An exclusive prefix sum
//! turns the counts into start offsets. The second pass parses the text again and drops every
//! edge into the next free slot of its node, which an atomic cursor hands out. This is a counting
//! sort by node, done in place: no list of parsed edges ever exists.
//!
//! ## Queries: bidirectional Dijkstra
//!
//! The reference answers each query with a Dijkstra search from `s` that stops when it settles
//! `t`. On a road network that search grows like a disc around `s` and settles every node closer
//! than `t`. Bidirectional Dijkstra ([`distance`]) grows one disc from `s` over the forward graph
//! and one from `t` over the reverse graph, and stops soon after they meet. Two discs of half the
//! radius cover about half the area of one full disc. Unreachable queries also end early: a
//! target with no in-edges empties the backward frontier after one step, where a forward search
//! would first settle the whole graph.
//!
//! Each side ([`Search`]) keeps a binary heap of `(distance, node)` pairs with lazy deletion, and
//! its tentative distances in a small hash map ([`Labels`]). A search therefore costs memory for
//! the nodes it touches, not for all `n`.
//!
//! ## Threads and output
//!
//! [`answer`] runs one worker per hardware thread. Each worker takes the next unanswered query
//! from a shared atomic counter (dynamic scheduling with a chunk size of one), so a few long
//! searches cannot leave the other threads idle. Every answer has its own slot, and `main` prints
//! the slots in input order, so the output never depends on which thread answered what.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::error::Error;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::thread;

// `mmap(2)` from the C library, which std links already. Declaring it here saves the `libc` crate.
unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
}
/// `mmap` protection: the pages may be read, and nothing else.
const PROT_READ: i32 = 1;
/// `mmap` flag: a private mapping. We never write to it, so it shares the page cache's pages.
const MAP_PRIVATE: i32 = 2;

/// The answer for an unreachable target, and the top of an empty heap. Real distances are below
/// 2^63 (spec §3), so they never collide with it, and it prints as `-1` once read as an `i64`.
const UNREACHABLE: u64 = u64::MAX;

/// Reads `graph.txt` and `queries.txt` from the data directory (first argument, default `/data`)
/// and writes `distances.txt`, one answer per query in input order, to the output directory
/// (second argument, default `/out`).
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let data_dir = args.next().unwrap_or_else(|| "/data".into());
    let out_dir = args.next().unwrap_or_else(|| "/out".into());
    let graph_txt = map_file(&Path::new(&data_dir).join("graph.txt"))?;
    let queries_txt = map_file(&Path::new(&data_dir).join("queries.txt"))?;

    // Header `n m`. A CSR edge packs its other end into 32 bits, hence the bound on n. The CSR
    // is sized by counting the edge lines, so m is not needed.
    let mut at = 0;
    let n = u32::try_from(number(graph_txt, &mut at)).map_err(|_| "n is over 2^32 - 1")?;
    number(graph_txt, &mut at);
    let threads = thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let (forward, reverse) = build_graph(&graph_txt[at..], n as usize, threads);

    // One `s t` line per query, no header (spec §2).
    let mut queries = Vec::new();
    let mut at = 0;
    while at < queries_txt.len() {
        queries.push((number(queries_txt, &mut at), number(queries_txt, &mut at)));
    }
    let answers = answer(&forward, &reverse, &queries, threads);

    let mut out = Vec::with_capacity(answers.len() * 12);
    for answer in answers {
        // UNREACHABLE is u64::MAX, whose bits read as -1 in an i64: the spec's `-1` with no
        // branch. Every real distance is below 2^63, so it prints unchanged.
        writeln!(out, "{}", answer.into_inner().cast_signed())?;
    }
    let out_path = Path::new(&out_dir).join("distances.txt");
    std::fs::write(&out_path, out).map_err(|e| format!("{}: {e}", out_path.display()))?;
    // Exit without running destructors: freeing the graph and search tables would only add to
    // the timed wall. The output is already written in full.
    std::process::exit(0)
}

/// Maps the whole file read-only and returns its bytes. The page cache is pre-warmed, so this
/// copies nothing and charges almost nothing to our memory cgroup, unlike reading the file into
/// the heap.
fn map_file(path: &Path) -> Result<&'static [u8], Box<dyn Error>> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let len = usize::try_from(file.metadata()?.len())?;
    if len == 0 {
        return Ok(&[]); // mmap rejects a zero length, and an empty queries.txt is legal (spec §2)
    }
    let fd = file.as_raw_fd();
    // SAFETY: a new read-only private mapping of an open fd; the kernel picks the address.
    let ptr = unsafe { mmap(std::ptr::null_mut(), len, PROT_READ, MAP_PRIVATE, fd, 0) };
    if ptr.addr() == usize::MAX {
        return Err(format!("mmap {}: {}", path.display(), io::Error::last_os_error()).into());
    }
    // SAFETY: the mapping holds `len` readable bytes and is never unmapped, so it outlives every
    // borrow. /data is mounted read-only, so the bytes cannot change under us.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Parses the decimal number at `*at` and moves `*at` past it and the one separator after it.
///
/// The spec guarantees the format (§2): unsigned decimals with no sign or leading zeros, and
/// exactly one space or LF between fields. So the parser needs no validation, and the same
/// function reads the header, the edge lines and the queries.
fn number(bytes: &[u8], at: &mut usize) -> u64 {
    let mut value = 0;
    while let Some(&digit @ b'0'..=b'9') = bytes.get(*at) {
        value = value * 10 + u64::from(digit - b'0');
        *at += 1;
    }
    *at += 1;
    value
}

/// Splits `text` into `parts` chunks of whole lines. Each split point starts at an even share of
/// the bytes and moves forward to the next line start, so every line lands in exactly one chunk.
fn line_chunks(text: &[u8], parts: usize) -> Vec<&[u8]> {
    let mut bounds = vec![0];
    for i in 1..parts {
        let at = text.len() * i / parts;
        let newline = text[at..].iter().position(|&b| b == b'\n');
        bounds.push(newline.map_or(text.len(), |p| at + p + 1));
    }
    bounds.push(text.len());
    bounds.windows(2).map(|w| &text[w[0]..w[1]]).collect()
}

/// Parses every `u v w` edge line and calls `visit(tail, head, weight)` on it, one scoped thread
/// per chunk. [`build_graph`] runs this twice, once to count and once to scatter.
fn for_each_edge(chunks: &[&[u8]], visit: &(impl Fn(usize, usize, u64) + Sync)) {
    thread::scope(|s| {
        for &chunk in chunks {
            s.spawn(move || {
                let mut at = 0;
                while at < chunk.len() {
                    let tail = number(chunk, &mut at) as usize;
                    let head = number(chunk, &mut at) as usize;
                    visit(tail, head, number(chunk, &mut at));
                }
            });
        }
    });
}

/// Builds the forward and the reverse [`Csr`] from the edge lines after the header, by a parallel
/// counting sort.
///
/// The first pass counts every node's out-degree and in-degree. [`first_slots`] turns the counts
/// into start offsets. The second pass scatters each edge into its node's next free slot, which
/// an atomic `fetch_add` on the node's cursor hands out, so two threads never get the same slot.
/// Edges land in whatever order the threads race to, and no distance depends on that order.
///
/// Every line is kept, as in the reference: of parallel edges only the lightest can ever improve
/// a label, a duplicate changes nothing, and a self-loop never improves one (spec §3).
fn build_graph(body: &[u8], n: usize, threads: usize) -> (Csr, Csr) {
    let chunks = line_chunks(body, threads);
    // Node u's count and cursor live at index u + 1. The prefix sum then puts u's first slot
    // there, and after the scatter it holds u's end, which is where u + 1 starts: the cursors
    // have become the CSR offsets, with index 0 still 0. No shift pass is needed.
    let (mut out_cursor, mut in_cursor) = (atomic_zeros(n + 1), atomic_zeros(n + 1));
    for_each_edge(&chunks, &|tail, head, _| {
        out_cursor[tail + 1].fetch_add(1, Relaxed);
        in_cursor[head + 1].fetch_add(1, Relaxed);
    });
    let out_slots = atomic_zeros(first_slots(&mut out_cursor));
    let in_slots = atomic_zeros(first_slots(&mut in_cursor));
    for_each_edge(&chunks, &|tail, head, weight| {
        // Weight and neighbour share one u64, so relaxing an edge costs one load (spec §2: weights
        // fit in 32 bits).
        let slot = out_cursor[tail + 1].fetch_add(1, Relaxed) as usize;
        out_slots[slot].store(weight << 32 | head as u64, Relaxed);
        let slot = in_cursor[head + 1].fetch_add(1, Relaxed) as usize;
        in_slots[slot].store(weight << 32 | tail as u64, Relaxed);
    });
    let forward = Csr::new(out_cursor, out_slots);
    (forward, Csr::new(in_cursor, in_slots))
}

/// `len` zeroed atomics. Atomics let the scatter threads share one array with no `unsafe`;
/// `Relaxed` is enough, since the scope's join orders every write before the searches read.
fn atomic_zeros(len: usize) -> Vec<AtomicU64> {
    (0..len).map(|_| AtomicU64::new(0)).collect()
}

/// Replaces each count with the sum of the counts before it (an exclusive prefix sum) and
/// returns the total. Applied to degrees, this gives every node the first slot of its edges.
fn first_slots(counts: &mut [AtomicU64]) -> usize {
    let mut total = 0;
    for count in counts {
        total += std::mem::replace(count.get_mut(), total);
    }
    total as usize
}

/// A directed graph in compressed sparse row form: one flat array of edges grouped by node, and
/// an array of where each group starts. A node's edges are one slice, with no pointer chasing.
struct Csr {
    /// Node u's edges are `edges[offsets[u]..offsets[u + 1]]`, so there are n + 1 entries.
    offsets: Vec<u64>,
    /// `weight << 32 | other_end`, where `other_end` is the head in the forward graph and the
    /// tail in the reverse graph.
    edges: Vec<u64>,
}

impl Csr {
    /// Unwraps the filled builder arrays into plain integers. `AtomicU64` and `u64` share a
    /// layout, so std's in-place iteration reuses each buffer and nothing is reallocated.
    fn new(offsets: Vec<AtomicU64>, slots: Vec<AtomicU64>) -> Self {
        let offsets = offsets.into_iter().map(AtomicU64::into_inner).collect();
        let edges = slots.into_iter().map(AtomicU64::into_inner).collect();
        Self { offsets, edges }
    }

    /// The packed edges of `node`.
    fn edges(&self, node: u32) -> &[u64] {
        let node = node as usize;
        &self.edges[self.offsets[node] as usize..self.offsets[node + 1] as usize]
    }
}

/// Answers every query on `threads` scoped threads and returns one slot per query, in input
/// order.
///
/// Each thread owns one [`Search`] per direction and reuses them across queries, so the heaps
/// and maps allocate only while they grow. Threads take the next unanswered query from a shared
/// counter instead of a fixed share, because query cost varies by orders of magnitude (the target
/// is drawn by Dijkstra rank, 2 to 131,071, spec §2).
fn answer(forward: &Csr, reverse: &Csr, queries: &[(u64, u64)], threads: usize) -> Vec<AtomicU64> {
    let answers = atomic_zeros(queries.len());
    let next = AtomicUsize::new(0);
    thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let (mut from_s, mut to_t) = (Search::new(forward), Search::new(reverse));
                loop {
                    let i = next.fetch_add(1, Relaxed);
                    let Some(&(s, t)) = queries.get(i) else { break };
                    let d = distance(&mut from_s, &mut to_t, s as u32, t as u32);
                    answers[i].store(d, Relaxed);
                }
            });
        }
    });
    answers
}

/// Returns d(s, t) by bidirectional Dijkstra (Pohl, 1971), or [`UNREACHABLE`].
///
/// `from_s` searches the forward graph from `s`, `to_t` searches the reverse graph from `t`, and
/// `best` is the length of the shortest s-t path seen so far. A path is seen when one side
/// improves the label of a node that the other side has labelled: the two labels join into a
/// path through that node ([`Search::step`]).
///
/// The search stops once the two heap tops sum to at least `best`, and `best` is then exact.
/// Suppose a shorter path P existed. Take the last node `a` on P whose distance from `s` is below
/// the forward top: `a` is settled forward, so its edge to the next node `b` on P was relaxed.
/// The rest of P from `b` is then shorter than the backward top, so `b` is settled backward. Both
/// sides have labelled `b`, with labels no worse than P's, so `best` is at most P's length, which
/// contradicts the assumption. The argument needs `best` to be updated on every label
/// improvement: updating it only when a node is settled by both sides misses paths through
/// weight-0 edges.
///
/// An empty heap's top is [`UNREACHABLE`], so the loop also ends when either side runs out of
/// nodes. That side has settled everything it can reach, so `best` is the answer, `UNREACHABLE`
/// included.
fn distance(from_s: &mut Search, to_t: &mut Search, s: u32, t: u32) -> u64 {
    from_s.start(s);
    to_t.start(t);
    // A path may have zero edges, even on a node with no edges (spec §3). For s == t, best starts
    // at 0 and the loop never runs, since no heap top is below 0. Starting from UNREACHABLE
    // instead would answer a round trip, or -1 on a node with no edges.
    let mut best = if s == t { 0 } else { UNREACHABLE };
    // Saturating, because an empty heap's top is u64::MAX.
    while from_s.top().saturating_add(to_t.top()) < best {
        // Advance the smaller frontier, not the one with the smaller top key: a source with no
        // in-edges leaves through edges near 2^32 (spec §2), and comparing keys would make the
        // other side settle the whole graph before the source side took a second step.
        if from_s.heap.len() <= to_t.heap.len() {
            from_s.step(&to_t.dist, &mut best);
        } else {
            to_t.step(&from_s.dist, &mut best);
        }
    }
    best
}

/// One direction of a bidirectional search: Dijkstra's algorithm over one [`Csr`], advanced one
/// settled node at a time by [`distance`]. A thread keeps one per direction for all its queries.
struct Search<'g> {
    /// The forward graph for the search from `s`, the reverse graph for the search from `t`.
    graph: &'g Csr,
    /// `(distance, node)` entries, smallest distance first. A node whose label improves gets a
    /// new entry and the old one stays behind (lazy deletion), because std's `BinaryHeap` has no
    /// decrease-key.
    heap: BinaryHeap<Reverse<(u64, u32)>>,
    /// Tentative distance of every node this search has labelled.
    dist: Labels,
}

impl<'g> Search<'g> {
    /// An idle search over `graph`, with an empty heap and no labels.
    fn new(graph: &'g Csr) -> Self {
        let (heap, dist) = Default::default();
        Self { graph, heap, dist }
    }

    /// Resets the search to a single label, `node` at distance 0. Clearing keeps the capacity
    /// the heap and the map grew to in earlier queries.
    fn start(&mut self, node: u32) {
        self.heap.clear();
        self.heap.push(Reverse((0, node)));
        self.dist.clear();
        self.dist.insert(node, 0);
    }

    /// The smallest distance in the heap, or [`UNREACHABLE`] when it is empty. No unsettled node
    /// is closer than this, which is what the stopping rule in [`distance`] relies on.
    fn top(&self) -> u64 {
        self.heap.peek().map_or(UNREACHABLE, |&Reverse((d, _))| d)
    }

    /// Settles the closest unsettled node and relaxes its edges. Every label it improves on a
    /// node that `other` has labelled joins the two searches into a path, which is offered to
    /// `best`.
    fn step(&mut self, other: &Labels, best: &mut u64) {
        // An entry above its node's label is stale (lazy deletion): the node was settled at a
        // shorter distance already, so the entry is dropped.
        if let Some(Reverse((d, node))) = self.heap.pop()
            && d == self.dist[&node]
        {
            for &edge in self.graph.edges(node) {
                let (next, via) = (edge as u32, d + (edge >> 32));
                let label = self.dist.entry(next).or_insert(UNREACHABLE);
                if via < *label {
                    *label = via;
                    self.heap.push(Reverse((via, next)));
                    if let Some(&rest) = other.get(&next) {
                        *best = (*best).min(via + rest);
                    }
                }
            }
        }
    }
}

/// Tentative distances of one search, keyed by node id.
///
/// A search touches thousands of the n nodes, not millions, so a hash map costs memory for those
/// only. A dense array would cost 8n bytes (14 MB at full scale) per thread and direction.
/// `clear` keeps the map's capacity, so after the first few queries it stops reallocating.
type Labels = HashMap<u32, u64, std::hash::BuildHasherDefault<NodeHash>>;

/// The hasher for [`Labels`]: Fibonacci hashing, one multiply by 2^64 divided by the golden ratio.
///
/// std's default `SipHash` resists hash flooding, which costs dozens of cycles per key, and none
/// of it is needed for node ids from a fixed file. The multiply keeps the low bits a bijection of
/// the id's low bits, which are already uniform because ids are a random relabelling (spec §2),
/// and mixes them into the high bits. In a local A/B on full seed 2 (4 shared E-cores), it cut
/// the whole run by 12 to 17% against `SipHash`.
#[derive(Default)]
struct NodeHash(u64);

impl std::hash::Hasher for NodeHash {
    /// The hash of the one id written since the hasher was created.
    fn finish(&self) -> u64 {
        self.0
    }

    /// Never called: [`Labels`] keys are `u32`, which hash through `write_u32`.
    fn write(&mut self, _: &[u8]) {
        unreachable!("labels hash only u32 node ids");
    }

    /// Hashes one node id.
    fn write_u32(&mut self, node: u32) {
        self.0 = u64::from(node).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}
