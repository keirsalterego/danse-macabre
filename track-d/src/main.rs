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
//! index of where each node's group starts and ends ([`Csr::rows`]), so a node's edges are one
//! contiguous slice. There are two of them: `forward` groups the edges by tail for the search
//! from `s`, and `reverse` groups them by head for the search from `t`.
//!
//! An edge is one `u32`: the neighbour in the low bits, as many as a node id needs (21 at full
//! scale), and the weight in the bits above it (11 at full scale, so up to 2,046). Road weights
//! are between 1 and a few thousand, median about 120 (spec §2), so nearly every weight fits.
//! The others set the field to all ones and live in a small side map ([`Csr::heavy`]). At 4
//! bytes an edge, the edges of the two CSRs take 40 MB at full scale instead of 80 MB.
//!
//! [`build_graph`] fills both with a parallel counting sort in three passes, one thread per chunk
//! of lines, and no atomics. Pass 1 counts each chunk's edges per block of 2,048 consecutive node
//! ids. Pass 2 parses the text again and writes every edge into a run of the final array that
//! belongs to its chunk and its block alone, and its node's place in the block into a parallel
//! array of `u16`. Pass 3 sorts each block, which fits in L2, by place. Besides the CSR, those
//! 2-byte places are the build's only big buffers.
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
//! Each side ([`Search`]) keeps its frontier in a bucket queue ([`Queue`]) with lazy deletion,
//! and its tentative distances in a small hash map ([`Labels`]). A search therefore costs memory
//! for the nodes it touches, not for all `n`.
//!
//! ## Threads and output
//!
//! [`answer`] runs one worker per hardware thread. Each worker takes the next unanswered query
//! from a shared atomic counter (dynamic scheduling with a chunk size of one), so a few long
//! searches cannot leave the other threads idle. Every answer has its own slot, and `main` prints
//! the slots in input order, so the output never depends on which thread answered what.

use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed, Ordering::SeqCst};
use std::{cmp::Reverse, error::Error, io, io::Write, os::fd::AsRawFd, path::Path, thread};

// `mmap(2)`, `madvise(2)` and `mallopt(3)` from the C library, which std links already. Declaring
// them here saves the `libc` crate. The constants are Linux's, from `<sys/mman.h>` and
// `<malloc.h>`; a comment at each call names them.
unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
    fn mallopt(param: i32, value: i32) -> i32;
}

/// The answer for an unreachable target, and the top of an empty queue. Real distances are below
/// 2^63 (spec §3), so they never collide with it, and it prints as `-1` once read as an `i64`.
const UNREACHABLE: u64 = u64::MAX;

/// Reads `graph.txt` and `queries.txt` from the data directory (first argument, default `/data`)
/// and writes `distances.txt`, one answer per query in input order, to the output directory
/// (second argument, default `/out`).
fn main() -> Result<(), Box<dyn Error>> {
    // M_MMAP_THRESHOLD (-3): glibc serves every allocation of at least this many bytes with its
    // own `mmap`, and unmaps it when it is freed. By default glibc raises that threshold to the
    // size of each mapped block it frees, up to 32 MB: here to 10 MB, when the build frees its
    // place arrays. The label tables then come from the heap, where each table a search outgrows
    // stays behind as free space that still counts as used. A fixed threshold unmaps them
    // instead. 64 KiB, half the default, also maps the tables of 8,192 buckets (74 KB) and up. On
    // seed 1 the peak fell from 91-96 MB to 82 MB, at the same wall time.
    // SAFETY: sets a malloc parameter; no other thread exists yet.
    unsafe { mallopt(-3, 64 << 10) };
    let arg = |i, default: &str| std::env::args().nth(i).unwrap_or(default.into());
    let (data_dir, out_dir) = (arg(1, "/data"), arg(2, "/out"));
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
    for_each_line(queries_txt, |[s, t]| queries.push((s as u32, t as u32)));
    let answers = answer(&[forward, reverse], graph_txt, &queries, threads);

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
    // PROT_READ (1): the pages may only be read. MAP_PRIVATE (2): we never write to them, so the
    // mapping shares the page cache's pages. mmap rejects a zero length, so an empty file (an
    // empty queries.txt is legal, spec §2) maps a page past its end, which nothing reads.
    // SAFETY: a new read-only private mapping of an open fd; the kernel picks the address.
    let ptr = unsafe { mmap(std::ptr::null_mut(), len.max(1), 1, 2, file.as_raw_fd(), 0) };
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
///
/// The fast path is SWAR, SIMD within a register: it reads 8 bytes as one little-endian `u64`
/// and handles all of their digits at once, with no branch per byte. A byte-at-a-time loop
/// mispredicts its exit once per number, since the lengths vary, and that cost more than the
/// arithmetic. Take `"904 7\n..."`:
///
/// 1. Every digit (0x30..=0x39) has bit 4 set, and the separators, space (0x20) and LF (0x0A),
///    have it clear. So the lowest clear bit 4 marks the end: here byte 3, so `len` is 3.
/// 2. The low nibbles of the 3 digit bytes, `[9, 0, 4]`, are shifted to the top of the word.
///    That drops the bytes after the number and leaves zero bytes below, which read as leading
///    zeros: the word now holds the digits `00000904`, first digit in the lowest byte.
/// 3. Three multiplies merge neighbours: digit pairs into 2-digit numbers (`00 00 09 04`), those
///    into 4-digit numbers (`0000 0904`), and those into the value, 904.
///
/// Numbers of 8 digits or more (in practice only weights near 2^32), and numbers that start in
/// the last 7 bytes of `bytes`, take the byte loop below.
fn number(bytes: &[u8], at: &mut usize) -> u64 {
    if let Some(&word) = bytes.get(*at..).and_then(<[u8]>::first_chunk::<8>) {
        let word = u64::from_le_bytes(word);
        let len = (!word & 0x1010_1010_1010_1010).trailing_zeros() / 8;
        if (1..8).contains(&len) {
            let digits = (word & 0x0F0F_0F0F_0F0F_0F0F) << (64 - 8 * len);
            let pairs = digits.wrapping_mul(10 << 8 | 1) >> 8 & 0x00FF_00FF_00FF_00FF;
            let quads = pairs.wrapping_mul(100 << 16 | 1) >> 16 & 0x0000_FFFF_0000_FFFF;
            *at += len as usize + 1;
            return quads.wrapping_mul(10_000 << 32 | 1) >> 32;
        }
    }
    let mut value = 0;
    while let Some(&digit @ b'0'..=b'9') = bytes.get(*at) {
        value = value * 10 + u64::from(digit - b'0');
        *at += 1;
    }
    *at += 1;
    value
}

/// Splits `text` into `parts` chunks of whole lines. Split point i starts at an even share of
/// the bytes, `at`, and moves forward to the first line start from there on: the byte after the
/// first LF from `at - 1` on. So every line lands in exactly one chunk.
fn line_chunks(text: &[u8], parts: usize) -> Vec<&[u8]> {
    let newline = |from: usize| text[from..].iter().position(|&b| b == b'\n');
    let split = |i| match text.len() * i / parts {
        0 => 0,
        at => newline(at - 1).map_or(text.len(), |p| at + p),
    };
    (0..parts).map(|i| &text[split(i)..split(i + 1)]).collect()
}

/// Parses every line of `N` numbers in `text`, such as an `s t` query or a `u v w` edge, and
/// calls `visit` on its numbers.
fn for_each_line<const N: usize>(text: &[u8], mut visit: impl FnMut([u64; N])) {
    let mut at = 0;
    while at < text.len() {
        visit(std::array::from_fn(|_| number(text, &mut at)));
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
/// 1. Each thread counts its chunk's edges per block of [`BLOCK`] nodes, by tail and by head. At
///    full scale that is 2 × 850 counters, which stay in L1.
/// 2. A prefix sum in (block, chunk) order gives every chunk its own run of slots inside every
///    block's range. Each thread parses its chunk again and appends every edge to its run
///    ([`append`]), and the edge's place in its block to the same slot of a separate array of
///    places. A thread writes to 2 × 850 runs of each, whose current cache lines fit in L2. Heavy
///    weights ([`Csr::heavy`]) also go to a list of the thread's own.
/// 3. Each block, about 6,000 edges or 23 KB at full scale, is counting-sorted by place on one
///    thread, which also writes the block's part of [`Csr::rows`] ([`sort`]).
///
/// For example, the line `5000 7 9` lands in forward block 2 at place 904 (5000 = 2 · 2048 +
/// 904), and in reverse block 0 at place 7. At full scale its forward edge is `9 << 21 | 7`.
///
/// Every line is kept, as in the reference: of parallel edges only the lightest can ever improve
/// a label, a duplicate changes nothing, and a self-loop never improves one (spec §3). Each
/// node's edges end up in an order set by the chunking, and no distance depends on it.
fn build_graph(body: &[u8], n: usize, threads: usize) -> Result<[Csr; 2], Box<dyn Error>> {
    let (chunks, blocks) = (line_chunks(body, threads), n.div_ceil(BLOCK));
    // Pass 1: counts[chunk][direction][block]. Direction 0 is keyed by the tail (the forward
    // graph), direction 1 by the head (the reverse graph).
    let mut counts = vec![[vec![0; blocks], vec![0; blocks]]; chunks.len()];
    let work = chunks.iter().zip(&mut counts);
    on_threads(work, |(chunk, [by_tail, by_head])| {
        for_each_line(chunk, |[tail, head, _]| {
            by_tail[tail as usize / BLOCK] += 1;
            by_head[head as usize / BLOCK] += 1;
        });
    });
    let in_block = |direction: usize, block: usize| counts.iter().map(move |c| c[direction][block]);
    let m: usize = (0..blocks).flat_map(|block| in_block(0, block)).sum();
    u32::try_from(m).map_err(|_| "over 2^32 - 1 edges")?;
    let id_bits = n.bit_width();
    let mut graphs = [(); 2].map(|()| Csr {
        rows: vec![0; n.div_ceil(4)],
        wide: HashMap::new(),
        edges: vec![0; m],
        id_bits,
        heavy: HashMap::default(),
    });
    let mut places = [(); 2].map(|()| vec![0; m]);
    // Zeroed allocations come from the kernel as untouched pages, which it zeroes on the first
    // write. When several threads first write to the same 2 MB huge page at once, it zeroes a page
    // for each of them and keeps one, and on seed 1 pass 2 took 19 ms instead of 13. So before
    // pass 2, one write per 2 MB faults the arrays in, each thread in its own share. Zeroing is
    // memory traffic, and half the threads did it faster than all of them or a third. Without huge
    // pages, these writes fault in a few 4 KB pages, and pass 2 faults in the rest as before.
    let share = (4 * m).div_ceil(threads).max(1);
    let edge_parts = graphs.iter_mut().flat_map(|g| g.edges.chunks_mut(share));
    let place_parts = places.iter_mut().flat_map(|p| p.chunks_mut(share));
    on_threads(edge_parts.zip(place_parts), |(edges, places)| {
        edges.iter_mut().step_by(1 << 19).for_each(|e| *e = 0);
        places.iter_mut().step_by(1 << 20).for_each(|p| *p = 0);
    });

    // Pass 2: runs[chunk][direction][block] is the chunk's run of slots in that block.
    let mut runs: Vec<[Vec<Run>; 2]> = chunks.iter().map(|_| Default::default()).collect();
    for (direction, (graph, places)) in graphs.iter_mut().zip(&mut places).enumerate() {
        let lens = || (0..blocks).flat_map(|block| in_block(direction, block));
        let edge_runs = split_runs(&mut graph.edges, lens());
        for (i, (edges, places)) in edge_runs.zip(split_runs(places, lens())).enumerate() {
            runs[i % chunks.len()][direction].push((edges.iter_mut(), places.iter_mut()));
        }
    }
    // The weight field's largest value marks a heavy edge, so it holds 0..mark.
    let mark = u64::from(u32::MAX) >> id_bits;
    let mut heavy = vec![Vec::new(); chunks.len()];
    let work = chunks.iter().zip(runs).zip(&mut heavy);
    on_threads(work, |((chunk, [mut by_tail, mut by_head]), heavy)| {
        for_each_line(chunk, |[tail, head, weight]| {
            if weight >= mark {
                heavy.push((tail as u32, head as u32, weight));
            }
            let field = weight.min(mark) << id_bits;
            append(&mut by_tail, tail as usize, (field | head) as u32);
            append(&mut by_head, head as usize, (field | tail) as u32);
        });
    });
    for &(tail, head, weight) in heavy.iter().flatten() {
        for (graph, ends) in graphs.iter_mut().zip([(tail, head), (head, tail)]) {
            let lightest = graph.heavy.entry(ends).or_insert(weight);
            *lightest = (*lightest).min(weight);
        }
    }

    // Pass 3: sort every block, dealt out to the threads in turn, one direction at a time, so the
    // first direction's places are freed before the second direction's rows take memory. The
    // rows are faulted in first, one write per 2 MB, for the same reason as the arrays above:
    // 12 threads writing to new pages at once raised the peak by up to 20 MB on seed 1.
    for (direction, (graph, mut places)) in graphs.iter_mut().zip(places).enumerate() {
        on_threads(graph.rows.chunks_mut(1 << 18), |rows| rows[0] = 0);
        let lens = || (0..blocks).map(|block| in_block(direction, block).sum());
        let block_runs = split_runs(&mut graph.edges, lens()).enumerate();
        let freed = AtomicUsize::new(places.as_ptr().addr().next_multiple_of(1 << 21));
        let block_places = block_runs.zip(split_runs(&mut places, lens()));
        let (mut work, mut first) = ((0..threads).map(|_| Vec::new()).collect::<Vec<_>>(), 0);
        for (((i, run), places), rows) in block_places.zip(graph.rows.chunks_mut(BLOCK / 4)) {
            let next = first + run.len() as u32;
            work[i % threads].push((run, places, rows, first, Vec::new()));
            first = next;
        }
        // Thread i sorts blocks i, i + threads, i + 2 * threads and so on, and after each one it
        // sets done[i] to where that block's places end. Every place below the least of them
        // then belongs to a sorted block, so the whole 2 MB pages there are given back at once
        // (a part of a huge page would stay charged). Else the first direction's rows add to
        // both directions' places, which was the run's peak: 1.9 MB lower on seed 1 (PERF.md).
        let done: Vec<_> = (0..threads).map(|_| AtomicUsize::new(0)).collect();
        on_threads(work.iter_mut().zip(&done), |(share, end)| {
            for (run, places, rows, first, wide) in share {
                sort(run, places, rows, *first, wide);
                end.store(places.as_ptr_range().end.addr(), SeqCst);
                let to = done.iter().map(|d| d.load(SeqCst)).min().unwrap_or(0) >> 21 << 21;
                let from = freed.fetch_max(to, Relaxed);
                // SAFETY: the range is empty or holds only places of sorted blocks, which nothing
                // reads again. MADV_DONTNEED (4) drops its pages.
                unsafe { madvise(from as *mut u8, to.saturating_sub(from), 4) };
            }
        });
        graph.wide = work.into_iter().flatten().flat_map(|work| work.4).collect();
    }
    Ok(graphs)
}

/// Cuts `slots` into consecutive runs of the given lengths, as they are asked for. Collecting
/// them first took 160 more page faults in pass 2's set-up, 0.5 ms on its one thread (PERF.md).
fn split_runs<T>(
    mut slots: &mut [T],
    lens: impl Iterator<Item = usize>,
) -> impl Iterator<Item = &mut [T]> {
    lens.map(move |len| slots.split_off_mut(..len).expect("lengths add up"))
}

/// Nodes per block, the unit of work of pass 3 in [`build_graph`]. 2,048 keeps a block's edges
/// (about 6,000, or 23 KB, at full scale) in L2 and its counters in L1, and it keeps the count of
/// runs a thread writes to in pass 2 small. A node's place in its block fits in a `u16`.
const BLOCK: usize = 2048;

/// A chunk's run of slots in one block during pass 2: its free edge slots and its free place
/// slots. Two iterators take 32 bytes against 48 for their `zip`, so a thread's 2 × 850 runs, which
/// pass 2 reads at random, take 54 KB instead of 82 KB.
type Run<'a> = (std::slice::IterMut<'a, u32>, std::slice::IterMut<'a, u16>);

/// Writes `edge` into the next slot of its block's run in `runs`, and the key node's place in its
/// block into the matching place slot. The key node is the tail in the forward graph and the head
/// in the reverse one.
fn append(runs: &mut [Run], key: usize, edge: u32) {
    let (edges, places) = &mut runs[key / BLOCK];
    *edges.next().expect("pass 1 counted it") = edge;
    *places.next().expect("pass 1 counted it") = (key % BLOCK) as u16;
}

/// Counting-sorts one block's edges by place and writes the block's groups of [`Csr::rows`] to
/// `rows`, and the rows of its wide groups to `wide`. `first` is the slot of `run[0]` in the whole
/// edge array.
fn sort(run: &mut [u32], places: &[u16], rows: &mut [u64], first: u32, wide: &mut Wide) {
    // Degrees first, then an exclusive prefix sum turns them into each node's next slot. Node k
    // counts in `next[k + 1]`, one place up, so that the loop below leaves `next[k]` where node
    // k's edges start.
    let mut next = vec![0; 4 * rows.len() + 1];
    for &place in places {
        next[usize::from(place) + 1] += 1;
    }
    next.iter_mut().fold(0, |s, c| s + std::mem::replace(c, s));
    // A copy to read from, since the loop overwrites `run` in sorted order.
    let packed = run.to_vec();
    for (edge, &place) in packed.into_iter().zip(places) {
        let slot = &mut next[usize::from(place) + 1];
        run[*slot as usize] = edge;
        *slot += 1;
    }
    // Node k's edges are now `run[next[k]..next[k + 1]]`, so a group of 4 is a window of 5.
    for (row, ends) in rows.iter_mut().zip(next.array_windows::<5>().step_by(4)) {
        let bytes = [1, 2, 3, 4].map(|i| (ends[i] - ends[0]).min(255) as u8);
        *row = u64::from(first + ends[0]) << 32 | u64::from(u32::from_le_bytes(bytes));
        if bytes[3] == 255 {
            let slot = |i: usize| (first + ends[i]) as usize;
            wide.extend((0..4).map(|i| ((*row, i as u32), (slot(i), slot(i + 1)))));
        }
    }
}

/// The rows of the wide groups of one block, as [`sort`] finds them, for [`Csr::wide`].
type Wide = Vec<((u64, u32), (usize, usize))>;

/// Asks the CPU to start loading the cache line that holds `item`, if there is one, into L1, and
/// returns without waiting for it.
fn prefetch<T>(item: Option<&T>) {
    let Some(item) = item else { return };
    let line = std::ptr::from_ref(item).cast();
    // SAFETY: a prefetch is a hint that never faults and changes no state, and `line` comes from a
    // reference, so it points into live memory anyway.
    unsafe { std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(line) };
}

/// A directed graph in compressed sparse row form: one flat array of edges grouped by node, and
/// an index of where each group starts and ends. A node's edges are one slice, with no pointer
/// chasing.
struct Csr {
    /// Where each node's edges are, 2 bytes a node in groups of 4: half of a `u32` offset per
    /// node, and 3.5 MB less per direction at full scale. The high half of `rows[u / 4]` is the
    /// slot of the group's first edge, a `u32` because [`build_graph`] checks that m fits, and
    /// byte `u % 4` of the low half is the end of node u's edges, counted from that slot. A group
    /// of 255 edges or more is wide: its last byte is 255 and its rows are in `wide`
    /// ([`Csr::row`]).
    rows: Vec<u64>,
    /// The start and end slot of each node in a wide group, by the group's `rows` entry and the
    /// node's place in the group.
    wide: HashMap<(u64, u32), (usize, usize)>,
    /// `weight << id_bits | other_end`, where `other_end` is the head in the forward graph and
    /// the tail in the reverse graph. A weight field of all ones marks a heavy edge.
    edges: Vec<u32>,
    /// Bits of the other end: every node id is below 2^`id_bits`, and the weight field is the
    /// `32 - id_bits` bits above them.
    id_bits: u32,
    /// The weights too big for the weight field, by (node, other end). Of parallel heavy edges it
    /// keeps the lightest, the only one a shortest path can use (spec §3). Hashed by [`NodeHash`],
    /// which [`build_graph`] fills it with in a third of the time `SipHash` took.
    heavy: HashMap<(u32, u32), u64, std::hash::BuildHasherDefault<NodeHash>>,
}

impl Csr {
    /// The start and end slot of `node`'s edges in `edges`, from its group in `rows`.
    fn row(&self, node: u32) -> (usize, usize) {
        let group = self.rows[node as usize / 4];
        if group as u32 >> 24 == 255 {
            // Lays the rare lookup and its hashing out of the search loop's way.
            std::hint::cold_path();
            return self.wide[&(group, node % 4)];
        }
        // Byte i of `group << 8` is where node i - 1's edges end, which is where node i's start.
        let at = |i: u32| (group >> 32) as usize + (group << 8 >> (8 * i) & 0xFF) as usize;
        (at(node % 4), at(node % 4 + 1))
    }

    /// The packed edges of `node`. A row of up to 8 edges, one 32-byte load, is copied into
    /// `copy` and returned from there. At full scale a node has 2.9 edges on average.
    ///
    /// The copy is for the hardware prefetchers. Read in place, a row is a run of 4-byte loads
    /// by one instruction, and on the i7-13620H we measure on, that sets the prefetchers off. On
    /// seed 1 they then brought twice as many lines into L2 (113M against 55M), 40% more loads
    /// missed L2, and the whole run took 5% longer. 8-byte edges did not set them off, and
    /// neither does the copy. The 32 bytes may reach into the next rows; the slice stops at the
    /// end of this one.
    fn edges<'a>(&'a self, node: u32, copy: &'a mut [u32; 8]) -> &'a [u32] {
        let (start, end) = self.row(node);
        if let Some(window) = self.edges.get(start..start + 8)
            && end - start <= 8
        {
            copy.copy_from_slice(window);
            return &copy[..end - start];
        }
        &self.edges[start..end]
    }

    /// Returns a function that unpacks one of `node`'s edges into its other end and its weight.
    /// It works out the field layout once, so a loop over the edges keeps it in registers.
    fn unpack(&self, node: u32) -> impl Fn(&u32) -> (u32, u64) {
        let (bits, mask) = (self.id_bits, (1 << self.id_bits) - 1);
        let all_ones = u64::from(u32::MAX) >> bits;
        move |&edge| {
            let (other, weight) = ((u64::from(edge) & mask) as u32, u64::from(edge) >> bits);
            if weight == all_ones {
                return (other, self.heavy_weight(node, other));
            }
            (other, weight)
        }
    }

    /// The weight of the heavy edge from `node` to `other`. `#[cold]` keeps this rare lookup and
    /// its hashing out of the search loop.
    #[cold]
    fn heavy_weight(&self, node: u32, other: u32) -> u64 {
        self.heavy[&(node, other)]
    }
}

/// Answers every query on `threads` scoped threads and returns one slot per query, in input
/// order.
///
/// Each thread owns one [`Search`] per direction and reuses them across queries, so the queues
/// and maps allocate only while they grow. Threads take the next unanswered query from a shared
/// counter instead of a fixed share, because query cost varies by orders of magnitude (the target
/// is drawn by Dijkstra rank, 2 to 131,071, spec §2).
///
/// `graphs` holds the forward and the reverse graph. `text` is graph.txt, which nothing reads
/// again: a thread that finds no query left drops its share of the pages from the page tables,
/// every `threads`-th piece of 8 MB. The process would otherwise unmap them all as it exits,
/// 1.3 to 2.5 ms on one thread on seed 1, while the threads that finish first wait up to 5 ms
/// for the last query.
fn answer(graphs: &[Csr], text: &[u8], queries: &[(u32, u32)], threads: usize) -> Vec<AtomicU64> {
    // Atomics let the threads share one answer array with no `unsafe`. `Relaxed` is enough, since
    // the threads are joined before `main` reads.
    let answers: Vec<_> = queries.iter().map(|_| AtomicU64::new(0)).collect();
    let next = AtomicUsize::new(0);
    on_threads(0..threads, |k| {
        let (mut from_s, mut to_t) = (Search::new(&graphs[0]), Search::new(&graphs[1]));
        loop {
            let i = next.fetch_add(1, Relaxed);
            let Some(&(s, t)) = queries.get(i) else { break };
            answers[i].store(distance(&mut from_s, &mut to_t, s, t), Relaxed);
        }
        for pages in text.chunks(1 << 23).skip(k).step_by(threads) {
            // SAFETY: the pages are an 8 MB aligned part of graph.txt's read-only mapping, which
            // nothing reads after the build. MADV_DONTNEED (4) drops them from the page tables.
            unsafe { madvise(pages.as_ptr().cast_mut(), pages.len(), 4) };
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
/// The search stops once the two queue tops sum to at least `best`, and `best` is then exact.
/// Suppose a shorter path P existed. Take the last node `a` on P whose distance from `s` is below
/// the forward top: `a` is settled forward, so its edge to the next node `b` on P was relaxed.
/// The rest of P from `b` is then shorter than the backward top, so `b` is settled backward. Both
/// sides have labelled `b`, with labels no worse than P's, so `best` is at most P's length, which
/// contradicts the assumption. The argument needs `best` to be updated on every label
/// improvement: updating it only when a node is settled by both sides misses paths through
/// weight-0 edges.
///
/// An empty queue's top is [`UNREACHABLE`], so the loop also ends when either side runs out of
/// nodes. That side has settled everything it can reach, so `best` is the answer, `UNREACHABLE`
/// included.
fn distance(from_s: &mut Search, to_t: &mut Search, s: u32, t: u32) -> u64 {
    // A path may have zero edges, even on a node with no edges (spec §3). For s == t, best starts
    // at 0 and the loop never runs, since no queue top is below 0. Starting from UNREACHABLE
    // instead would answer a round trip, or -1 on a node with no edges.
    let mut best = if s == t { 0 } else { UNREACHABLE };
    let (mut top_f, mut top_b) = (from_s.start(s), to_t.start(t));
    // Saturating, because an empty queue's top is u64::MAX.
    while top_f.saturating_add(top_b) < best {
        // Advance the smaller frontier, not the one with the smaller top key: a source with no
        // in-edges leaves through edges near 2^32 (spec §2), and comparing keys would make the
        // other side settle the whole graph before the source side took a second step.
        if from_s.queue.len <= to_t.queue.len {
            top_f = from_s.step(&to_t.dist, &mut best);
        } else {
            top_b = to_t.step(&from_s.dist, &mut best);
        }
    }
    best
}

/// One direction of a bidirectional search: Dijkstra's algorithm over one [`Csr`], advanced one
/// settled node at a time by [`distance`]. A thread keeps one per direction for all its queries.
struct Search<'g> {
    /// The forward graph for the search from `s`, the reverse graph for the search from `t`.
    graph: &'g Csr,
    /// One entry per label, smallest distance first. A node whose label improves gets a new
    /// entry and the old one stays behind (lazy deletion).
    queue: Queue,
    /// Tentative distance of every node this search has labelled.
    dist: Labels,
}

impl<'g> Search<'g> {
    /// An idle search over `graph`, with an empty queue and no labels.
    fn new(graph: &'g Csr) -> Self {
        let (queue, dist) = (Queue::new(), Labels::default());
        Self { graph, queue, dist }
    }

    /// Resets the search to a single label, `node` at distance 0, and returns the new top of its
    /// queue, 0. Clearing keeps the capacity the queue and the map grew to in earlier queries.
    fn start(&mut self, node: u32) -> u64 {
        self.queue.clear();
        self.queue.push(0, node);
        self.dist.0.clear();
        self.dist.1.clear();
        self.dist.0.insert(node, 0);
        self.queue.top()
    }

    /// Settles the node on top of the queue, which [`Queue::top`] found, and relaxes its edges.
    /// Every label it improves on a node that `other` has labelled joins the two searches into a
    /// path, which is offered to `best`. Returns the new top of its queue: a step moves no other
    /// top, so [`distance`] keeps the other side's.
    fn step(&mut self, other: &Labels, best: &mut u64) -> u64 {
        // A stale entry, one above its node's label, is settled again with no check. The label's
        // own entry came out first and relaxed the same edges from a shorter distance, so no label
        // improves now: nothing is pushed and `best` stays. The check would cost a lookup a step.
        let (d, node) = self.queue.pop();
        // Software pipelining. Settling a node takes two dependent trips to DRAM: its group in
        // `rows`, then its edges, both at random spots in big arrays. The queue knows its next
        // pops (a push below may still cut in), so this fetches the edges of the node two pops
        // ahead and the group of the node three ahead. The previous step fetched the next node's
        // edges. Of the depths tried, this one measured fastest. At full scale a node has 2.9
        // edges on average, 12 bytes, so the first 64-byte line of a row usually holds them all
        // (prefetching the last line as well measured no faster even with 8-byte edges). The
        // group read for it can miss too, but nothing waits for it.
        if let [_, Some(second), third] = self.queue.upcoming(d) {
            prefetch(self.graph.edges.get(self.graph.row(second).0));
            prefetch(third.map(|third| &self.graph.rows[third as usize / 4]));
        }
        let (unpack, mut copy) = (self.graph.unpack(node), [0; 8]);
        for (next, weight) in self.graph.edges(node, &mut copy).iter().map(&unpack) {
            let via = d + weight;
            // A new label starts at `u32::MAX`, which a `via` below 2^32 - 1 beats with one compare.
            let label = self.dist.0.entry(next).or_insert(u32::MAX);
            let long = || self.dist.1.get(&next).is_none_or(|&old| via < old);
            if via < u64::from(*label) || *label == u32::MAX && long() {
                *label = via.min(u64::from(u32::MAX)) as u32;
                if *label == u32::MAX {
                    self.dist.1.insert(next, via);
                }
                self.queue.push(via, next);
                if let Some(&rest) = other.0.get(&next) {
                    let short = (rest < u32::MAX).then_some(u64::from(rest));
                    *best = (*best).min(via + short.unwrap_or_else(|| other.1[&next]));
                }
            }
        }
        self.queue.top()
    }
}

/// Buckets in a [`Queue`]'s ring, a multiple of 64. Road weights are mostly a few hundred and
/// at most a few thousand (spec §2), so nearly every push lands in the ring. 1,024 and 2,048
/// buckets measured 2 to 3 ms slower on seed 1.
const RING: usize = 4096;

/// A bucket queue (Dial, 1969) of `(distance, node)` entries. Dijkstra's keys are monotone: a
/// step settles a node at the smallest key `cur` and pushes `cur + weight`, never less.
///
/// An entry with a key in `cur..cur + RING` goes into bucket `key % RING` of a fixed ring, one
/// key per bucket, and the ring is scanned upwards from `cur`. A key beyond that goes into a
/// binary heap, `far`. A weight can be anything below 2^32 (spec §2), so a ring that covered
/// every key would need 2^32 buckets; this way a long edge costs one heap push instead.
///
/// Push and pop are O(1) plus a scan of the occupancy bits to the next full bucket, where a
/// binary heap sifts through up to 11 levels, each a branch on the data. The queue also knows
/// which nodes it pops next ([`Queue::upcoming`]), which [`Search::step`] prefetches. Against
/// that heap on seed 1 (12 threads), the query phase fell from 120 to 93 ms.
struct Queue {
    /// The entry on top of each bucket's stack, or [`NIL`].
    heads: Vec<u32>,
    /// Bit `b % 64` of word `b / 64` is set when bucket `b` is not empty.
    full: [u64; RING / 64],
    /// Every bucket's stack entries, as `[node, next entry]`, in one array. Popped entries form a
    /// free list from `free`, so the array only grows to the most entries queued at once.
    links: Vec<[u32; 2]>,
    free: u32,
    /// The key [`Queue::top`] found last: no entry is below it.
    cur: u64,
    /// Entries pushed beyond the ring.
    far: BinaryHeap<Reverse<(u64, u32)>>,
    /// Entries queued, stale ones included.
    len: usize,
}

/// The end of a bucket's stack, and of the free list.
const NIL: u32 = u32::MAX;

impl Queue {
    fn new() -> Self {
        Self {
            heads: vec![NIL; RING],
            full: [0; RING / 64],
            links: Vec::new(),
            free: NIL,
            cur: 0,
            far: BinaryHeap::new(),
            len: 0,
        }
    }

    /// Empties the queue. It keeps its capacity.
    fn clear(&mut self) {
        self.heads.fill(NIL);
        self.links.clear();
        self.far.clear();
        (self.full, self.free, self.cur, self.len) = ([0; RING / 64], NIL, 0, 0);
    }

    /// Queues `node` at distance `key`, which is at least `cur`.
    ///
    /// LLVM keeps this out of line, so every relaxation in [`Search::step`] paid for a call.
    /// Forced inline, a run on seed 1 executes 1.7% fewer instructions and takes 2% fewer cycles
    /// (PERF.md, step 16).
    #[inline(always)]
    fn push(&mut self, key: u64, node: u32) {
        self.len += 1;
        if key - self.cur >= RING as u64 {
            return self.far.push(Reverse((key, node)));
        }
        let b = key as usize % RING;
        let entry = [node, self.heads[b]];
        if let Some(&[_, next_free]) = self.links.get(self.free as usize) {
            self.links[self.free as usize] = entry;
            self.heads[b] = std::mem::replace(&mut self.free, next_free);
        } else {
            self.heads[b] = self.links.len() as u32;
            self.links.push(entry);
        }
        self.full[b / 64] |= 1 << (b % 64);
    }

    /// The first key from `from` on whose bucket is full, scanning the ring once at most.
    fn ahead(&self, from: u64) -> Option<u64> {
        // The word and the bit of bucket `from % RING`; RING is a multiple of 64.
        let (w, bit) = (from as usize % RING / 64, from as usize % 64);
        if self.full[w] >> bit != 0 {
            return Some(from + u64::from((self.full[w] >> bit).trailing_zeros()));
        }
        (1..=RING / 64).find_map(|i| {
            let word = self.full[(w + i) % (RING / 64)];
            (word != 0).then(|| from + (i * 64 - bit) as u64 + u64::from(word.trailing_zeros()))
        })
    }

    /// The next three nodes [`Queue::pop`] returns from key `key` on, unless a push cuts in. Only
    /// a hint for prefetching: it skips `far`, and when it runs past the ring it starts over.
    fn upcoming(&self, mut key: u64) -> [Option<u32>; 3] {
        let mut link = NIL;
        std::array::from_fn(|_| {
            if link == NIL {
                key = self.ahead(key)?;
                link = self.heads[key as usize % RING];
                key += 1;
            }
            let [node, next] = self.links[link as usize];
            link = next;
            Some(node)
        })
    }

    /// The smallest key queued, or [`UNREACHABLE`] when the queue is empty. It becomes `cur`. No
    /// unsettled node is closer than this, which is what the stopping rule in [`distance`] relies
    /// on.
    fn top(&mut self) -> u64 {
        let far = self.far.peek().map_or(UNREACHABLE, |r| r.0.0);
        self.cur = self.ahead(self.cur).unwrap_or(UNREACHABLE).min(far);
        self.cur
    }

    /// Removes an entry at `cur`, the key the last [`Queue::top`] found, and returns it.
    fn pop(&mut self) -> (u64, u32) {
        self.len -= 1;
        let b = self.cur as usize % RING;
        let head = self.heads[b];
        let Some(&[node, next]) = self.links.get(head as usize) else {
            return (self.cur, self.far.pop().expect("top found a key").0.1);
        };
        self.heads[b] = next;
        // Clears the bucket's bit if that was its last entry.
        self.full[b / 64] &= !(u64::from(next == NIL) << (b % 64));
        self.links[head as usize][1] = std::mem::replace(&mut self.free, head);
        (self.cur, node)
    }
}

/// Tentative distances of one search, keyed by node id: the label as a `u32` in the first map,
/// or, for a label of 2^32 - 1 or more, `u32::MAX` there and the label in the second.
///
/// A search touches thousands of the n nodes, not millions, so a hash map costs memory for those
/// only. A dense array would cost 4n bytes (7 MB at full scale) per thread and direction.
/// `clear` keeps the map's capacity, so after the first few queries it stops reallocating.
///
/// Only paths over the edges near 2^32 (spec §2) get labels that long, so the second map is
/// nearly always empty, and a lookup reads it only behind a `u32::MAX`. A slot of the first map
/// takes 9 bytes with its control byte, against 13 for a `u64` label packed to 4-byte alignment.
/// Every table grows to the largest search its thread ran, about 17.5 MB over the 24 at full
/// scale with `u64` labels, so this cuts the end of the queries by about 5 MB.
type Labels = (Short, HashMap<u32, u64>);

/// The first map of [`Labels`].
type Short = HashMap<u32, u32, std::hash::BuildHasherDefault<NodeHash>>;

/// The hasher for [`Labels`] and [`Csr::heavy`]: Fibonacci hashing, one multiply by 2^64 divided
/// by the golden ratio.
///
/// std's default `SipHash` resists hash flooding, which costs dozens of cycles per key, and none
/// of it is needed for node ids from a fixed file. The multiply keeps the low bits a bijection of
/// the id's low bits, which are already uniform because ids are a random relabelling (spec §2),
/// and mixes them into the high bits. In a local A/B on full seed 2 (4 shared E-cores), it cut
/// the whole run by 12 to 17% against `SipHash`.
#[derive(Default)]
struct NodeHash(u64);

impl std::hash::Hasher for NodeHash {
    /// The hash of the ids written since the hasher was created.
    fn finish(&self) -> u64 {
        self.0
    }

    /// Hashes one more node id into the state. [`Labels`] keys are `u32`, and std's `write_u32`
    /// passes their 4 bytes here; a [`Csr::heavy`] key passes two. For one id the state is 0, so
    /// the xor is the id itself.
    fn write(&mut self, id: &[u8]) {
        let id = u32::from_ne_bytes(id.try_into().expect("a u32 node id"));
        self.0 = (self.0 ^ u64::from(id)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}
