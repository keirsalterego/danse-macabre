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
//! [`build_graph`] fills both with a parallel counting sort in three passes, one thread per chunk
//! of lines, and no atomics. Pass 1 counts each chunk's edges per block of 2,048 consecutive node
//! ids. Pass 2 parses the text again and writes every edge into a run of the final array that
//! belongs to its chunk and its block alone. Pass 3 sorts each block, which fits in L2, by node.
//! No list of parsed edges ever exists next to the CSR.
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
    let [forward, reverse] = build_graph(&graph_txt[at..], n as usize, threads)?;

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

/// Parses every `u v w` edge line of one chunk and calls `visit(tail, head, weight)` on it.
fn for_each_edge(chunk: &[u8], mut visit: impl FnMut(usize, usize, u64)) {
    let mut at = 0;
    while at < chunk.len() {
        let tail = number(chunk, &mut at) as usize;
        let head = number(chunk, &mut at) as usize;
        visit(tail, head, number(chunk, &mut at));
    }
}

/// Runs `work` on every item, each on its own scoped thread, and returns once all are done.
fn on_threads<T: Send>(items: impl IntoIterator<Item = T>, work: impl Fn(T) + Sync) {
    let work = &work;
    thread::scope(|s| {
        for item in items {
            s.spawn(move || work(item));
        }
    });
}

/// Builds the forward and the reverse [`Csr`] from the edge lines after the header, by a parallel
/// counting sort in three passes that share no counter between threads.
///
/// The textbook parallel counting sort counts every node's degree with atomic adds, turns the
/// counts into offsets, and drops each edge into its node's next free slot with another atomic
/// add. Each of those adds hits a random cache line of a 14 MB array, and a `lock`ed add on x86
/// also waits for every store before it, so the scatter runs at memory latency, one edge at a
/// time per thread. In the profile they took 80% of the build's CPU time. Here every thread owns
/// its counters and its slots:
///
/// 1. Each thread counts its chunk's edges per [`Blocks`] block, by tail and by head. At full
///    scale that is 2 × 850 counters, which stay in L1.
/// 2. A prefix sum in (block, chunk) order gives every chunk its own run of slots inside every
///    block's range. Each thread parses its chunk again and appends every edge to its run
///    ([`Blocks::append`]), with the edge's place in its block parked in spare bits. A thread
///    writes to 2 × 850 runs, whose current cache lines fit in L2.
/// 3. Each block, about 6,000 edges or 47 KB at full scale, is counting-sorted by place on one
///    thread, which also writes the block's offsets ([`Blocks::sort`]).
///
/// For example, with 2,048 nodes per block the line `5000 7 9` lands in forward block 2 at place
/// 904 (5000 = 2 · 2048 + 904), and in reverse block 0 at place 7.
///
/// Every line is kept, as in the reference: of parallel edges only the lightest can ever improve
/// a label, a duplicate changes nothing, and a self-loop never improves one (spec §3). Each
/// node's edges end up in an order set by the chunking, and no distance depends on it.
fn build_graph(body: &[u8], n: usize, threads: usize) -> Result<[Csr; 2], Box<dyn Error>> {
    let chunks = line_chunks(body, threads);
    let blocks = Blocks::new(n);
    // Pass 1: counts[chunk][direction][block]. Direction 0 is keyed by the tail (the forward
    // graph), direction 1 by the head (the reverse graph).
    let mut counts = vec![[vec![0; blocks.len], vec![0; blocks.len]]; chunks.len()];
    let work = chunks.iter().zip(&mut counts);
    on_threads(work, |(chunk, [by_tail, by_head])| {
        for_each_edge(chunk, |tail, head, _| {
            by_tail[tail >> blocks.bits] += 1;
            by_head[head >> blocks.bits] += 1;
        });
    });
    let in_block = |direction: usize, block: usize| counts.iter().map(move |c| c[direction][block]);
    let m: usize = (0..blocks.len).flat_map(|block| in_block(0, block)).sum();
    let edge_count = u32::try_from(m).map_err(|_| "over 2^32 - 1 edges")?;
    // Zeroed allocations come from the kernel as untouched pages, so nothing zeroes 80 MB up
    // front: each page is faulted in by the thread that first writes to it in pass 2.
    let mut graphs = [(); 2].map(|()| Csr {
        offsets: vec![0; n + 1],
        edges: vec![0; m],
    });

    // Pass 2: runs[chunk][direction][block] is the chunk's run of slots in that block.
    let mut runs: Vec<[Vec<&mut [u64]>; 2]> = chunks.iter().map(|_| Default::default()).collect();
    for (direction, graph) in graphs.iter_mut().enumerate() {
        let lens = (0..blocks.len).flat_map(|block| in_block(direction, block));
        for (i, run) in split_runs(&mut graph.edges, lens).into_iter().enumerate() {
            runs[i % chunks.len()][direction].push(run);
        }
    }
    let work = chunks.iter().zip(runs);
    on_threads(work, |(chunk, [mut by_tail, mut by_head])| {
        for_each_edge(chunk, |tail, head, weight| {
            blocks.append(&mut by_tail, tail, head, weight);
            blocks.append(&mut by_head, head, tail, weight);
        });
    });

    // Pass 3: sort every block of both directions, split evenly over the threads.
    let mut work = Vec::with_capacity(2 * blocks.len);
    for (direction, graph) in graphs.iter_mut().enumerate() {
        graph.offsets[n] = edge_count;
        let lens = (0..blocks.len).map(|block| in_block(direction, block).sum());
        let block_offsets = graph.offsets[..n].chunks_mut(1 << blocks.bits);
        let block_runs = split_runs(&mut graph.edges, lens);
        let mut first = 0;
        for (run, offsets) in block_runs.into_iter().zip(block_offsets) {
            let next = first + run.len() as u32;
            work.push((run, offsets, first));
            first = next;
        }
    }
    let per_thread = work.len().div_ceil(threads);
    on_threads(work.chunks_mut(per_thread), |share| {
        for (run, offsets, first) in share {
            blocks.sort(run, offsets, *first);
        }
    });
    Ok(graphs)
}

/// Cuts `slots` into consecutive runs of the given lengths.
fn split_runs<T>(mut slots: &mut [T], lens: impl IntoIterator<Item = usize>) -> Vec<&mut [T]> {
    let cut = |len| {
        let (run, rest) = std::mem::take(&mut slots).split_at_mut(len);
        slots = rest;
        run
    };
    lens.into_iter().map(cut).collect()
}

/// Node ids cut into blocks of 2^`bits` consecutive ids, the unit of work of pass 3 in
/// [`build_graph`].
#[derive(Clone, Copy)]
struct Blocks {
    /// Bits of a node id: every id is below 2^`id_bits`.
    id_bits: u32,
    /// log2 of the nodes per block.
    bits: u32,
    /// The number of blocks, n / 2^`bits` rounded up.
    len: usize,
}

impl Blocks {
    /// Picks 2,048 nodes per block. That keeps a block's edges (about 6,000, or 47 KB, at full
    /// scale) in L2 and its counters in L1, and it keeps the count of runs a thread writes to in
    /// pass 2 small. A node's place in its block must also fit next to a node id in 32 bits
    /// ([`Blocks::append`]), which only shrinks the blocks when n is over 2^21.
    ///
    /// shortcut: smaller blocks mean more counters and runs per chunk, n / 2^`bits` of each. That
    /// is fine to about 2^24 nodes (the spec's full scale is 1,740,512); far beyond it, pass 2
    /// would need wider edges that keep the whole key node.
    fn new(n: usize) -> Self {
        let id_bits = n.saturating_sub(1).bit_width();
        let bits = (32 - id_bits).min(11);
        let len = n.div_ceil(1 << bits);
        Self { id_bits, bits, len }
    }

    /// Writes one edge into the first slot of its block's run in `runs` and moves that run's
    /// start past it. The key node is the tail in the forward graph and the head in the reverse
    /// one. The edge is packed as in the final [`Csr`], the weight in the high half and the other
    /// end in the low bits, plus the key node's place in its block just above the other end.
    ///
    /// At full scale ids take 21 bits, so for `5000 7 9` the forward edge is
    /// `9 << 32 | 904 << 21 | 7`: place 904 sits in bits 21..32, which pass 3 reads and clears.
    fn append(self, runs: &mut [&mut [u64]], key: usize, other: usize, weight: u64) {
        let run = &mut runs[key >> self.bits];
        let (slot, rest) = std::mem::take(run)
            .split_first_mut()
            .expect("pass 1 counted it");
        let place = key & ((1 << self.bits) - 1);
        *slot = weight << 32 | (place << self.id_bits | other) as u64;
        *run = rest;
    }

    /// Counting-sorts one block's packed edges by place, clears the places, and gives each of the
    /// block's nodes its first slot in `offsets`. `first` is the slot of `run[0]` in the whole
    /// edge array.
    fn sort(self, run: &mut [u64], offsets: &mut [u32], first: u32) {
        let place = |edge: u64| (edge >> self.id_bits) as usize & ((1 << self.bits) - 1);
        // Degrees first, then an exclusive prefix sum turns them into each node's next slot.
        let mut next = vec![0; offsets.len()];
        for &edge in &*run {
            next[place(edge)] += 1;
        }
        let mut at = 0;
        for (offset, next) in offsets.iter_mut().zip(&mut next) {
            *offset = first + at;
            at += std::mem::replace(next, at);
        }
        // A copy to read from, since the loop overwrites `run` in sorted order.
        let packed = run.to_vec();
        for edge in packed {
            let slot = &mut next[place(edge)];
            run[*slot as usize] = edge & !(((1 << self.bits) - 1) << self.id_bits);
            *slot += 1;
        }
    }
}

/// `len` zeroed atomics. Atomics let the query threads share one answer array with no `unsafe`;
/// `Relaxed` is enough, since the scope's join orders every write before `main` reads.
fn atomic_zeros(len: usize) -> Vec<AtomicU64> {
    (0..len).map(|_| AtomicU64::new(0)).collect()
}

/// A directed graph in compressed sparse row form: one flat array of edges grouped by node, and
/// an array of where each group starts. A node's edges are one slice, with no pointer chasing.
struct Csr {
    /// Node u's edges are `edges[offsets[u]..offsets[u + 1]]`, so there are n + 1 entries. They
    /// are `u32`, half the size of `usize`, because [`build_graph`] checks that m fits.
    offsets: Vec<u32>,
    /// `weight << 32 | other_end`, where `other_end` is the head in the forward graph and the
    /// tail in the reverse graph.
    edges: Vec<u64>,
}

impl Csr {
    /// Starts loading the first cache line of `node`'s edges into L1, without waiting for it.
    /// At full scale a node has 2.9 edges on average, 23 bytes, so one 64-byte line usually
    /// holds them all: prefetching the last line as well measured no faster. The offsets entry
    /// read here can miss too, but nothing waits for it.
    fn prefetch(&self, node: u32) {
        if let Some(edge) = self.edges.get(self.offsets[node as usize] as usize) {
            let line = std::ptr::from_ref(edge).cast();
            // SAFETY: a prefetch is a hint that never faults and changes no state, and `line`
            // comes from a reference, so it points into live memory anyway.
            unsafe { std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(line) };
        }
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
            // Software pipelining. The node settled next is usually the one now on top of the
            // heap, and its edges sit at a random spot in a 40 MB array, a trip to DRAM. Starting
            // that load now overlaps it with the work on `node`'s edges below, where it would
            // otherwise stall the next step. This one prefetch cut the run by 28% (12 threads).
            if let Some(&Reverse((_, top))) = self.heap.peek() {
                self.graph.prefetch(top);
            }
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
