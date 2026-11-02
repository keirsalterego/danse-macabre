# Track D: performance log

Every kept step names its board run. A number marked local is from my laptop (i7-13620H, turbo off), comparing two versions run side by side; experiments that didn't stay are under the step they were tried after.

| | Reference (step 0) | Final code (step 20), two board runs |
|---|---|---|
| T | 72.69 s | 118.4 ms (run-4e0f7e6bd2df), 121.8 ms (run-56ed132a3ad9) |
| M | 893 MB | 65.7 MB, 65.8 MB |
| S (lines) | 43 | 424 |
| Points | 0 | 73.6, 73.3 |

## Checking the headline

Points are 50 · log10 K, where K = (Tref/T)^0.5 · min(Mref/M, 50)^0.2 · min(Sref/S, 1)^0.15 · (Dref/D)^0.15. For run-4e0f7e6bd2df: (72.69 / 0.118391)^0.5 = 24.78, (893 / 65.69)^0.2 = 1.685, (43 / 424)^0.15 = 0.709, and 1 for the dependencies. K = 29.6, and 50 · log10 29.6 = 73.58.

## What didn't work

- Relabeling nodes in BFS order (on step 7). Queries got faster, 93.8 → 84.8 ms, but computing the order took 80 ms and permuting the graph another 52 ms.
- Transparent huge pages off. 227 to 240 ms against 211 to 230 ms. The CSR was already mostly on huge pages, and its random reads miss the TLB more on 4 KB pages. Track C went the other way: turning them off took its peak from 39.0 to 15.47 MB.

## How I worked

D was my insurance track. I planned it small, so that one failed hash elsewhere would still leave three scored tracks, and Sref is only 43 here, which makes every line expensive. That's why steps 8, 14, 15 and 18 only remove lines. Every push passed all 522 cases of my gate first.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 72.69 | 893 | 0 |
| 1 | Rust v1: forward and reverse CSR built by a counting sort with atomic adds, and a bidirectional Dijkstra per query on 12 threads (475265e) | run-3d72cc2b20e3 | 0.438 | 141 | 58.2 |
| 2 | Counting sort with no atomics (482dcb3), then a prefetch of the next node's edges (c13e637), two steps deep (599190a) | run-97e259c08579 | 0.1889 | 128.2 | 66.7 |
| 3 | Numbers parsed eight bytes at a time (85568b4) | run-49af01c44df1 | 0.1665 | 125.8 | 68.0 |
| 4 | Heap entries ordered by one packed 128-bit key (542bc46) | run-05e531e8754c | 0.1565 | 125.2 | 68.6 |
| 5 | Edges packed into 4 bytes and read through a 32-byte copy, and glibc's mmap threshold fixed at 64 KiB (680f4ad) | run-74c555d4ba4f | 0.1551 | 83.9 | 70.2 |
| 6 | The build's arrays faulted in before pass 2, one write per 2 MB on half the threads (3d3e358; measured together with step 7) | run-bb1fd514f3b2 | 0.1279 | 81.4 | 71.6 |
| 7 | Each search's binary heap replaced by a bucket queue: a ring of 4,096 one-unit buckets found through a bitmap, with a small heap for keys past the ring (fc4ed14) | run-bb1fd514f3b2 | 0.1279 | 81.4 | 71.6 |
| 8 | The query code shortened by 24 lines at the same speed: `answer` reuses `on_threads`, and the queue's helpers are folded together | run-966433fb4d85 | 0.1264 | 84.0 | 71.8 |
| 9 | Each node's edge range kept in 2 bytes instead of a 4-byte offset, in groups of 4 nodes, and pass 3 run one direction at a time with its index faulted in first | run-ee72c614f94f | 0.1297 | 75.5 | 71.8 |
| 10 | Label distances aligned to 4 bytes, so a label table entry takes 12 bytes instead of 16 | run-37fab0938045 | 0.1280 | 69.4 | 72.3 |
| 11 | Each queue's top kept between steps; a step recomputes only its own side's | run-37fab0938045 | 0.1280 | 69.4 | 72.3 |
| 12 | Pass 2's runs kept as two slice iterators instead of their `zip`: 32 bytes a run instead of 48 | run-0581a96cbf3c | 0.1259 | 70.7 | 72.4 |
| 13 | Labels as `u32` distances, with the rare longer ones behind a `u32::MAX` in a second map | run-954b4981ddcb | 0.1255 | 68.1 | 72.7 |
| 14 | 11 fewer lines with the same machine code: the single-item `use` lines in one, the `mmap` and `mallopt` constants written at their one call each with their names in a comment, the file descriptor passed straight to `mmap`, and two pairs of variables declared together | run-954b4981ddcb | 0.1255 | 68.1 | 72.7 |
| 15 | 1 fewer line with the same machine code: `sort`'s counters and its running sum declared together | run-5194056b1310 | 0.1244 | 68.4 | 72.7 |
| 16 | `Queue::push` forced inline, so a relaxation no longer pays for a call: 1.8% fewer instructions | run-fc6e22635c4b | 0.1221 | 67.7 | 73.0 |
| 17 | Pass 3 gives back the places of sorted blocks as it goes: the threads take the blocks in turn, and the 2 MB pages below the slowest one's progress are dropped with `madvise` | run-d253110190f5 | 0.1276 | 66.0 | 72.6 |
| 18 | 30 fewer lines at the same speed: the Dockerfile without `WORKDIR`, one parser for edge and query lines, the runs cut with `split_off_mut`, the prefetches through one helper that takes an `Option`, and smaller cuts | run-e464a45452b1 | 0.1244 | 65.9 | 73.1 |
| 19 | The query threads that run out of queries drop graph.txt's pages from the page tables, so the exit no longer unmaps them on one thread | run-f3c4ed0faa3c | 0.1246 | 65.6 | 73.1 |
| 20 | The build's serial parts shortened: `split_runs` hands out the runs as they are asked for instead of collecting them, and the heavy-edge maps hash with `NodeHash` instead of `SipHash` | run-3416bb90eca1 | 0.1221 | 64.9 | 73.3 |

Local numbers are for full seed 1 unless a seed is named. "P-cores" means CPUs 0 to 11 of an
i7-13620H (its six P-cores and their hyperthreads), with all 12 threads and turbo off. Each figure
is `perf stat -r 5`, and variants were interleaved over three rounds. Peak memory is `memory.peak` of a
systemd-run scope on CPUs 0 to 11, with the inputs already in the page cache. A single board run is
noisy for this track: v1 scored 380, 438 and 500 ms, and step 3's code scored 166.5 ms here and
165.2 ms in run-321ee44c222c. So each step below says which local numbers back it.

## Step 2a: a counting sort with no atomics (482dcb3)

v1 counted degrees with one atomic add per edge and direction, then gave out slots with another.
Each add hit a random line of a 14 MB cursor array. On x86 a `lock`ed add also waits for every
store before it, so the scatter ran at one memory miss at a time per thread. The profile had
`fetch_add` at 26.7% of all CPU time, about 80% of the build. v1 also zeroed 80 MB of edge slots on
one thread before the scatter.

Now each thread counts its chunk's edges per block of 2,048 node ids in private counters. A prefix
sum gives every chunk its own run of slots in every block, so the second pass writes with plain
stores. A third pass counting-sorts each block (about 47 KB, so it stays in L2) by node. Offsets
became `u32`, and the edge arrays come zeroed from the kernel instead of being cleared.

- P-cores: elapsed 394 to 410 → 289 to 296 ms, task-clock 4.34 to 4.45 → 3.25 to 3.33 s, cycles 10.1 to 10.4
  → 7.4 to 7.6 G.
- Build phase on P-cores: about 60 ms after the change (21 + 31 + 8 ms for the three passes).
- Peak memory: 139.7 to 142.2 → 126.0 to 129.9 MB, from the narrower offsets. S 217 → 291.

## Step 2b: prefetch the next node's edges (c13e637)

After this change the queries were 80% of the run, and two dependent cache misses dominated each
settled node: its offsets entry, then its edge row, both at random spots in big arrays. The edge
row load alone was 22% of all samples and the offsets read 10%. The node settled next is usually the
one on top of the heap, so `step` now starts loading that node's edges before it relaxes the
current node's, which hides the wait behind that work.

- P-cores: elapsed 289 to 291 → 202 to 207 ms, task-clock 3.26 → 2.26 to 2.27 s, cycles 7.45 → 5.06 to 5.07
  G.
- Peak memory unchanged. S 291 → 300.

## Step 2c: two steps deep (599190a)

With the edge rows prefetched, the biggest stall became the offsets read for that prefetch (17.5%
of samples). The node after next is usually one of the heap root's two children, so `step` also
prefetches their offsets entries. One step later the top's offsets are already in cache.

- P-cores: elapsed 202 to 204 → 188 to 190 ms, task-clock 2.26 to 2.27 → 2.11 s, cycles 5.06 to 5.08 → 4.70
  G.
- Seed 2: 204 → 194 ms. Seed 3: 218 → 198 ms.
- Peak memory unchanged. S 300 → 306.

## Step 3: numbers parsed eight bytes at a time (85568b4)

The byte-at-a-time parser was about 16% of all samples. Its loop runs once per digit and exits on
the first non-digit, so the exit branch follows the varying number lengths. The new parser reads 8 bytes
as one `u64` and finds the separator from bit 4, which every digit has and space and LF don't. It
then merges up to 7 digits with three multiplies and no branch. Numbers of 8 or more digits take
the old loop; on seed 1 that is 50 of the 5M weights.

- P-cores: elapsed 193 to 216 → 167 to 190 ms, task-clock 2.09 to 2.12 → 1.82 to 1.84 s, cycles 4.66 to 4.71 →
  4.03 to 4.05 G, instructions 6.59 → 5.69 G.
- Parse passes: 21 → 9 ms and 31 → 20 ms. The build went from 60 to 37 ms.
- Seed 2: 194 → 168 ms. Seed 3: 201 → 175 ms.
- Peak memory unchanged. S 306 → 317.

## Step 4: heap entries as one 128-bit key (542bc46)

Binary-heap operations were 35% of all samples, hash-map probes 23%. Counters on seed 1:

- 7.57M pops, of which 0.45M were stale;
- 20.97M relaxations and 8.65M pushes;
- the heap never held more than 2,719 entries (43 KB).

So the heap's cost was compares and branches, not memory. Each entry is now `distance << 32 | node`
in one `u128`. A `(u64, u32)` tuple compares the distance, then on a tie the node, which takes
branches. A `u128` compares with one subtract-with-borrow and no branch.

- P-cores: elapsed 165 to 169 → 157 to 161 ms, task-clock 1.82 to 1.84 → 1.73 to 1.74 s, cycles 4.04 to 4.06 →
  3.80 to 3.81 G, instructions 5.69 → 5.27 G.
- The heap's share of samples went from 35.0% to 28.2%.
- Seed 2: 171 → 161 ms. Seed 3: 180 → 165 ms.
- Peak memory unchanged. S 317 → 323.

## Step 5: 4-byte edges and a fixed mmap threshold (680f4ad)

Where step 4's peak went, from `memory.stat` sampled through a seed 1 run: 123.6 of its 127.2 MB
were anonymous memory, and the peak came near the end of the queries. Of that:

- the two CSRs, 2 × (7 MB of offsets + 40 MB of edges) = 94 MB;
- the label maps, about 22 MB: of the 24 searches' maps, 17 had grown to 57,344 entries (1.1 MB)
  and 7 to 28,672;
- 5 to 10 MB of free heap space, left behind by label maps that grew;
- the heaps, about 1 MB.

The build's own buffers were already small (the edge arrays come zeroed from the kernel, and pass 2
parked the places in spare bits), so this step cuts the CSRs and the free space:

1. An edge is one `u32`: the other end in the low 21 bits at full scale, the weight in the 11 bits
   above it. A weight that doesn't fit (3,496 of the 5M on seed 1, among them the heavy edges near
   2^32) sets the field to all ones and goes into a small map. The CSRs take 54 MB instead of 94 MB.
   With no spare bits left, pass 2 writes each edge's place in its block to a `u16` array, which is
   freed after pass 3.
2. Read in place, a 4-byte row is a run of loads by one instruction, 4 bytes apart, and on the
   i7-13620H that sets off the hardware prefetchers: on seed 1 they brought 113M lines into L2
   instead of 55M, and the run took 5% longer. The same records in 8-byte slots did not set them off
   (55M). So each row of up to 8 edges is first copied to the stack with one 32-byte load.
3. Freeing the 10 MB place arrays raised glibc's mmap threshold to 10 MB. The label maps then came
   from the heap, where each map a search outgrew stayed behind as free space. A
   `mallopt(M_MMAP_THRESHOLD, 64 KiB)` at the start fixes the threshold, so an outgrown map is
   unmapped.

- P-cores: elapsed 158 to 162 → 164 to 167 ms, task-clock 1.75 to 1.78 → 1.80 to 1.82 s, cycles 3.82 to 3.88 →
  3.97 to 4.01 G, instructions 5.27 → 5.50 G.
- Peak memory: 126.6 to 129.2 → 81.7 to 81.9 MB.
- Seed 2: 159 → 164 to 166 ms, peak 123.0 to 123.5 → 81.7 to 84.2 MB. Seed 3: 164 → 170 to 171 ms, peak
  123.5 to 125.5 → 80.1 to 83.6 MB.
- Knockouts in the same session: without the row copy, 172 to 177 ms, task-clock 1.90 to 1.95 s, peak
  81.0 to 82.3 MB; without the fixed threshold, 164 to 165 ms, task-clock 1.78 to 1.79 s, peak 90.6 to 95.6 MB.
- S 323 → 352.

## Step 6: the build's arrays faulted in before pass 2

Round 7 looked at the build. Phase marks on seed 1 (medians of 7 runs on P-cores) split the 157 ms
before the output is written into:

- pass 1, 9.5 ms;
- pass 2's runs, set up on the main thread, 1.2 ms;
- pass 2, 19 ms;
- pass 3, 6.6 ms;
- the heavy edges and freeing the places, under 1 ms;
- the queries, 120 ms.

Outside `main`, the process takes about 1 ms to start and 2 ms at exit to unmap graph.txt.

Pass 2 parses the same text as pass 1 and writes each edge twice, once per direction, yet took twice
as long. In a run that stopped after the build, 0.21 to 0.24 G of its 0.95 G cycles were in the kernel,
71% of those in `clear_page_erms`, which zeroes new pages. Pass 2 is the first write to the 60 MB of
edge and place arrays, and every thread writes into every block, so all 12 threads fault on each new
2 MB huge page at once. Each of them zeroes a huge page; the kernel maps one and frees the rest.

Now, before pass 2, half the threads each write one value per 2 MB of their own share of the arrays,
so each huge page is faulted in by one thread, bar those that straddle two shares.

- Phase marks (seed 1, 7 runs each, interleaved): pass 2 18.7 → 13.0 ms, plus 3.3 ms for the new
  step, so 18.7 → 16.2 ms in all, and 156.7 → 154.5 ms before the output. A second session: 19.3 →
  16.4 ms, and 158.9 → 155.6 ms.
- P-cores, seed 1: elapsed 164.7 to 165.8 → 161.2 to 167.9 ms (medians 165.3 → 163.7), task-clock
  1.80 to 1.82 → 1.74 to 1.76 s.
- Seed 2: medians 170.4 → 166.5 ms, task-clock 1.85 to 1.87 → 1.81 to 1.82 s. Seed 3: 173.6 to 175.8 →
  170.1 to 172.1 ms, task-clock 1.90 to 1.91 → 1.84 to 1.85 s.
- With huge pages off (`PR_SET_THP_DISABLE`), the new step takes 0.3 ms and pass 2 faults its 4 KB
  pages as before: 20.8 → 21.2 ms for both, and elapsed medians 173.9 → 173.1 ms. The first
  version wrote once per 4 KB page. That cost 1.6 ms with huge pages off (20.8 → 22.4 ms), and with
  them on it did no better (16.5 against 16.2 ms).
- This laptop runs Debian 13 with huge pages on for all anonymous memory (`always`), and the
  scoring box runs Debian 13 too.
- Peak memory unchanged: seed 1 82.2 to 83.8 → 83.3 to 85.4 MB, seed 2 79.8 to 84.1 → 79.4 to 85.3 MB.
- S 352 → 359.

## Step 7: a bucket queue instead of the binary heap

A profile of the queries on step 5 put about 30% of each search step in the heap's sift-down
after a pop (11 dependent levels) and about 25% in label lookups. Prefetching already hid most
graph misses: loading edge rows was about 5% of query samples.

Each `Search` now keeps its frontier in a `Queue`, a bucket queue in the style of Dial's
algorithm:

- A key in `cur..cur + 4096`, where `cur` is the last key popped, goes into bucket `key % 4096` of
  a fixed ring. Keys only grow during a search and every edge weight is non-negative, so a pop
  takes the lowest non-empty bucket at or after `cur`, which a 64-word bitmap finds.
- A bucket holds one key, so its LIFO stack stores nodes only: `[node, next]` entries linked
  through one shared arena with a free list, so the arena only grows to the most entries queued at
  once. A vector per bucket was slower than the heap, from its cache misses.
- A key at or past the end of the ring goes to a small binary heap, `far`. Weights go up to
  2^32-1 (spec §2), and the near-2^32 edges in every seed take this path. The ring's size is a
  constant, not something read from the input, so a graph with only huge weights stays correct
  and pays one heap push per long edge.
- The queue knows the next nodes it will pop, so each step prefetches the edge row of the node two
  pops ahead and the offsets entry of the node three ahead. That replaces the guess at the heap's
  children.
- A stale pop (a node already settled at a smaller key) skips the label lookup that used to check
  it: its relaxations can never improve a label, so it pushes nothing and leaves `best` alone.

Balancing the two sides by queue length and the stopping rule are unchanged.

Measured on P-cores, interleaved, medians:

| Run | Queries before → after (ms) | Full run before → after (ms) |
|---|---|---|
| Seed 1, quiet machine, 11 runs (on step 5) | 119.75 → 93.11 | 164.69 → 136.48 |
| Seed 1, busy machine, 11 runs (on step 5) | 124.08 → 102.08 | 174.55 → 147.86 |
| Seed 2, busy machine, 7 runs (on step 5) | 129.06 → 106.14 | 175.10 → 151.98 |
| Seed 3, busy machine, 7 runs (on step 5) | 144.63 → 113.68 | 196.23 → 169.70 |
| Seed 1, 11 runs, on step 6 | | 160.02 → 133.06 |

Peak memory is unchanged within noise: median 83.2 → 82.8 MB over 9 runs each. S 359 → 450.

## Step 8: 24 fewer lines in the query code

Lines count on D more than on any other track (S 450 against a reference of 43), so a pass over
the query code removed what no longer paid for itself. `answer` uses the `on_threads` helper
instead of its own scope-and-spawn loop. `Search::top` only forwarded to `Queue::top`, so it is
gone. `Queue::upcoming` returns the next three nodes as an array instead of an iterator.
`Queue::clear` resets every bucket head with one `fill`. The rare `far` heap holds `(key, node)`
tuples instead of a packed `u128`. The node hasher hashes in `write` alone. Every doc comment
stayed.

S 450 → 426. Interleaved full runs, medians (fc4ed14 → step 8): seed 1 133.14 → 133.61,
133.77 → 133.52 and 135.11 → 135.41 ms (11 runs each), seed 2 139.92 → 137.68 ms and seed 3
141.70 → 140.94 ms (9 runs each). Peak memory median 83.7 → 82.9 MB (7 runs each).

## Step 9: a 2-byte row index, and pass 3 one direction at a time

Round 8 looked at peak memory, 81 to 84 MB on the board. A time series of the run's cgroup memory on
seed 1 put the peak near the end of the queries, at 82 to 83 MB:

- the two CSRs, 53.9 MB: 2 × 20 MB of edges and 2 × 7 MB of offsets;
- the label maps, 22.3 to 22.8 MB: each of the 24 keeps the size of the largest search it has run,
  and by the end 17 have 65,536 buckets (1.11 MB) and 7 have 32,768;
- queues, glibc's arenas, kernel memory and the cgroup's per-CPU charge stock, about 5 MB.

The build peaked at the end of pass 3, at about 78 MB: 40 MB of edges, 20 MB of places and the
14 MB of offsets that pass 3 writes for the first time. Those writes come from 12 threads at once,
and with shares of about 580 KB, 3 or 4 threads fault on each fresh huge page, so the race step 6
describes for pass 2 added spikes of 13 to 25 MB there.

Two changes:

- Pass 3 sorts one direction at a time, so the forward places are freed before the reverse index
  is written, and it faults each direction's index in first, one write per 2 MB, as step 6 does
  for the other arrays.
- `Csr::offsets`, a `u32` per node, became `Csr::rows`, a `u64` per group of 4 nodes: the slot of
  the group's first edge in the high half, and in each byte of the low half where one node's edges
  end, counted from that slot. That is 2 bytes a node instead of 4, 7 MB less at full scale. A
  group with 255 edges or more keeps its nodes' rows in a small map instead, and its last byte,
  255, sends lookups there. No seed has one (the most edges in a group on seed 1 is 22), but any
  graph stays correct. `sort` counts each node's degree one slot up, so the counting sort leaves
  an offsets array for the block, and each group is a window of 5 of it.

Peak memory, `memory.peak` in MB, 5 runs each, alternating:

| Seed | Step 8 | Pass 3 change alone | Step 9 |
|---|---|---|---|
| 1 | 81.3 to 82.9, median 82.3 | 80.7 to 82.7, median 81.5 | 74.8 to 76.9, median 74.9 |
| 2 | 80.0 to 84.2, median 84.0 | 78.2 to 79.7, median 78.7 | 70.5 to 72.3, median 71.7 |

The peak stays near the end of the queries (74 to 77 MB at 123 to 125 ms in three time series), and
the build now tops out at 67 to 68 MB.

Time, on a busy machine (load 17 to 22 from other work on CPUs 12 to 15):

- `ab.py`, alternating full runs, medians, step 8 → step 9: seed 1 151.9 → 151.4 ms (21 runs) and
  142.8 → 144.7 ms (41 runs); seed 2 161.1 → 155.7 and 143.1 → 141.8 ms; seed 3 141.9 → 144.2
  and 152.1 → 151.0 ms.
- `perf stat -r 5` on P-cores, 4 rounds: seed 1 elapsed medians 137.2 → 137.3 ms, cycles
  3.26 to 3.28 → 3.31 to 3.33 G, instructions 4.75 → 5.04 G; seed 2 144.3 → 141.6 ms, cycles 3.37 to 3.45 →
  3.44 to 3.48 G. Decoding the groups costs 6% more instructions, which the cache misses around
  them hide.

Correctness: the gate passes, 509 of 509. No seed has a wide group, so 200 generated graphs check
them against step 8: 60 random graphs with hub nodes, and 140 with one group of 4 hubs whose every
edge leads to or from a node of its own, so each answer says which hub owns the edge. They cover
groups of 254 and 255 edges, empty nodes, the first and last group of a block, and a last group cut
short by n. All 200 outputs are byte-identical to step 8's, and with the wide rows emptied, 54 of
the 60 random graphs changed.

S 426 → 443.

## Step 10: label distances aligned to 4 bytes

After step 9 the label tables were the biggest item at the peak after the graph. Each of the 24
keeps the size of the largest search its thread ran, about 23 MB in all at full scale. A
`HashMap<u32, u64>` entry is 16 bytes, 4 of them padding after the key. `Labels` now maps to
`Dist`, a `u64` in a `#[repr(C, packed(4))]` struct, so an entry is 12 bytes, and a slot with its
control byte 13 instead of 17.

- Peak memory, 5 runs each, alternating: seed 1 74.4 to 76.6 → 69.6 to 71.7 MB (medians 75.8 → 71.3),
  seed 2 70.5 to 73.1 → 67.6 to 68.8 MB (medians 71.5 → 67.8).
- `ab.py`, alternating full runs, medians, step 9 → step 10 (31 runs each): seed 1 136.1 → 134.7
  and 136.7 → 136.8 ms, seed 2 137.9 → 135.8 and 137.3 → 136.1 ms, seed 3 142.7 → 141.4 ms.
- `perf stat -r 5` on P-cores, seed 1, 3 rounds: task-clock 1.430 to 1.451 → 1.416 to 1.431 s, cycles
  3.25 to 3.30 → 3.22 to 3.27 G, instructions 5.04 → 5.11 G.
- S 443 → 445.

## Step 11: the queue tops kept between steps

Steps 10 and 11 went to the board in one push, so both rows cite run-37fab0938045.

A time split of step 7 on seed 1: the build takes 35 to 43 ms, the queries 93 to 105 ms, the output
0.08 ms. The search loop asked both queues for their top on every iteration, but a step moves
only its own side's top. `step` now returns its queue's new top, and `distance` keeps the other
side's. The side that did not step has an unchanged queue, so its top and the bucket cursor that
`pop` relies on are still valid, and the stop rule sees the same values as before.

`ab.py` against step 9, alternating full runs on seed 1, five sessions on a busy machine (load
9 to 12), medians: 158.5 → 155.4, 142.9 → 139.7, 139.6 → 137.9, 138.8 → 134.3 and 142.2 → 139.8 ms,
faster on median and minimum in all five, about -2.1% on average. The query phase alone, two
sessions of 21 runs: 96.8 → 94.3 and 97.9 → 94.6 ms. S 445 → 447. Gate: 509 pass, 0 fail.

Measured and not taken: the last threads idle for 3 to 5 ms at the end, because the top 5% of
queries use 53% of the CPU and their cost can't be predicted, so no cheap reordering helps.
Label and queue resets cost about 0.5 ms of wall time, and the 20 duplicate queries of a seed
about 0.7 ms, which would take about 10 lines to remove.

## Step 12: pass 2's runs as two iterators

A phase split of step 11's build on seed 1, medians of 25 runs: pass 1 9.5 ms, faulting the
arrays in 3.3, the runs set-up 1.4, pass 2 12.9, the heavy edges 0.4, pass 3 3.3 and 3.5. The
queries took 91.3 ms. Pass 1 is bound by instruction throughput, about 91 instructions per line.
Pass 2 parses the same text again and makes 4 scattered stores per line; a faster parse did not
shorten it (see the AVX2 row below).

Pass 2 appends every edge to one run in each direction, picked by its block, so a thread reads
and writes its 2 × 850 runs at random. A run was a `zip` of the edge-slot and place-slot
iterators, which keeps an index and two lengths besides the two iterators: 48 bytes. The two
iterators alone take 32, so a thread's runs take 54 KB instead of 82 KB, and `append` advances
each iterator itself.

Phase marks, 25 rounds each on seeds 1 and 2, medians: runs set-up 1.36 → 1.14 and
1.35 → 1.13 ms, pass 2 12.90 → 12.63 and 12.93 → 12.43 ms. `ab.py` against step 11, 41
alternating runs per binary and session, medians: seed 1 133.04 → 131.35, 132.56 → 130.69 and
133.92 → 131.67 ms, seed 2 133.50 → 131.97, 133.83 → 132.26 and 139.53 → 137.15 ms, seed 3
139.91 → 137.89 ms. About 1.2 ms of that is code placement. With every function aligned to 64
bytes in both builds (`-C llvm-args=-align-all-functions=6`, for this control only), the gain was
0.2, 0.6, 0.8 and 1.3 ms in four sessions, which matches the phase marks.

Peak memory, medians: seed 1 70.3 → 70.35 MB (10 runs each), seed 3 69.6 → 69.5 and seed 4
67.7 → 67.9 (5 runs each). Seed 2 rose from 67.6 to 68.25 MB over 10 runs. Its build maximum
stayed at 67.4 to 68.0 MB, but its query phase ended about 1 MB higher in anonymous memory in three
traced runs. The query code is unchanged, so this is most likely where the heap's pieces landed.
S 447 → 448. Gate: 509 pass, 0 fail.

## Step 13: labels as `u32` distances

After step 12 the peak still came near the end of the queries, and the 24 label tables made about
17.5 MB of it: 13 bytes a bucket, 17 tables at 65,536 buckets and 7 at 32,768. A label is now a
`u32` in the first of two maps, so a bucket takes 9 bytes. A label of 2^32-1 or more, which only
a path over one of the edges near 2^32 can have, is `u32::MAX` in the first map and its full value
in the second.

A new label starts at `u32::MAX`, so a `via` below 2^32-1 beats it with the one compare a
relaxation already made, and a miss in the first map is final. The second map is read only when
`via` itself reaches 2^32-1, and for the other side's label only behind a `u32::MAX`. On seed 1
the other side has the node in 5,031 of the 8.65M lookups.

- Peak memory, 6 alternating runs each, seed 1: 69.86 to 71.17 → 67.34 to 68.29 MB, medians 70.30 →
  67.70. In a trace of `memory.current` the queries now end at about 64 MB, and the peak moved to
  pass 3 of the forward direction, while the edges, both place arrays and the forward rows are
  alive. So 2.6 of the 5 MB show.
- `ab.py`, alternating runs, medians (minimums). 41 runs per binary on a quiet machine: seed 1
  131.91 → 129.91 (129.43 → 127.95) ms, seed 2 135.69 → 132.73 (131.67 → 130.21) ms. 31 runs per
  binary on a machine busy with other work: seed 1 143.00 → 140.84 (137.64 → 134.68) and
  149.72 → 150.48 (142.40 → 139.80) ms, seed 2 144.80 → 142.43 (138.61 → 136.59) ms, seed 3
  153.32 → 151.24 (140.77 → 140.55) ms.
- `perf stat -r 5` on P-cores, seed 1: task-clock 1.547 → 1.528 s, cycles 3.69 → 3.65 G,
  instructions 5.15 → 5.20 G. The smaller tables miss the caches less: at 65,536 buckets, the four
  tables of a core's two threads take 2.4 MB instead of 3.4 MB.
- S 448 → 453. Gate: 509 pass, 0 fail. Fresh seeds 5 to 9, 31 and 47: byte-identical to the
  reference.

## Step 14: 11 fewer lines

The six single-item `use` lines become one; `M_MMAP_THRESHOLD`, `PROT_READ` and `MAP_PRIVATE`
are written as numbers at their one call each, with their names and meaning in the comment above
it; `map_file` passes `file.as_raw_fd()` straight to `mmap`; and `main` and `build_graph` each
declare a pair of variables together. `.text` and `.rodata` are byte-identical to step 13's, so T
and M cannot move. S 453 → 442. Gate: 509 pass, 0 fail.

Steps 13 and 14 went to the board in one push, so both rows cite the same run.

## Step 15: 1 fewer line

`sort` declares its counters and its running sum together. `.text` and `.rodata` are
byte-identical to step 14's. S 442 → 441. Gate: 509 pass, 0 fail.

## Step 16: the queue's push inline

LLVM kept `Queue::push` out of line, so every relaxation in `Search::step` paid for a call and
its spills. `#[inline(always)]` puts it in the loop (pedantic clippy's `inline_always` is allowed
in `Cargo.toml`, with the reason). Local, AC power, turbo off, seed 1, 12 threads: `perf stat`,
5 runs, twice: instructions 5.188 → 5.093 G and 5.206 → 5.112 G (-1.8%), cycles 3.485 → 3.310 G
and 3.555 → 3.442 G, task-clock 1,458 → 1,384 ms and 1,487 → 1,440 ms. `ab.py`, interleaved
against step 15, medians (minimums): 31 runs, 132.43 → 130.56 ms (130.38 → 126.92); 41 runs,
135.63 → 130.98 (128.91 → 126.39); 41 runs, 133.52 → 130.68 (128.87 → 126.43); and on a first
build, 31 runs twice, 131.09 → 127.92 and 132.38 → 128.69. memory.peak cannot move, as `push`
runs only in the queries, and did not: 6 alternating runs, median 67.90 → 67.89 MB, highest
68.17 → 68.22. S 441 → 442. Gate: 512 pass, 0 fail (full seeds 1 to 12 and every tiny
seed).

## After step 16: where memory.peak comes from, and label changes that did not pay

Four query-side changes and one build-side change, each measured against step 16 (or step 15
where noted). None paid, so the code is step 16's. Rows in the table below.

Where memory.peak comes from, read from the cgroup at phase marks (seed 1, 12 threads, 3
runs, peak 66,044 to 66,516 KiB): it is reached at the end of pass 3's first direction. Then both
edge arrays (38.15 MiB), both place arrays (19.07 MiB) and the forward rows (3.32 MiB, written as
4 KB pages by all 12 threads) are live at once, with about 4.4 MiB on top: the process and its
wrapper, page tables (520 KiB), slab, and the memory cgroup's per-CPU charge batches. The
queries top out lower, at about 62,100 KiB (an earlier `memory.current` trace), with 12 threads'
label tables grown to 32,768 or 65,536 buckets. So a cut to the labels alone cannot move M until
the build peak falls by about 4.4 MiB. It also explains the measured peaks below: u16 labels
save 2 to 3 MB in the queries and nothing in the peak.

## Step 17: pass 3 gives back the places it has sorted

A phase trace of step 16 (seed 1, `memory.current` and `memory.peak` of the cgroup at each phase
mark, KiB, 3 runs) put the peak at the end of pass 3's first direction, 65,856 to 66,048, against
62,356 to 62,388 at the end of pass 2 and 60,488 to 62,632 when the query threads finish. The difference
is the forward rows, written while both directions' places are still alive, plus the sort's
buffers.

Pass 3 now deals the blocks out in turn: thread i sorts blocks i, i + 12, i + 24 and so on, in
order, and after each block stores where that block's places end. Below the least of the 12
stored ends every place belongs to a sorted block (a block that reaches below it is behind its
own thread's last one), so after each block the whole 2 MB pages below it that are not yet
dropped go back with `madvise(MADV_DONTNEED)`. The bounds are 2 MB aligned because a dropped part
of a huge page stays charged until the rest of it goes. The reverse direction runs the same code.

- Phase trace after (seed 1, 5 runs, KiB): pass 2 ends at 62,192 to 62,872, the forward pass 3 tops
  out at 63,436 to 64,448, and the query threads end at 62,948 to 63,652. The forward pass still adds
  about 1.2 MiB: the first huge page of places goes only once the slowest thread is past it, and
  the 4 KB pages before it stay.
- memory.peak, MiB, 6 alternating runs each: seed 1 63.9 to 64.4 → 61.9 to 62.7 (medians 64.1 →
  62.25) and 64.0 to 64.6 → 61.8 to 62.4 (64.3 → 62.15); seed 2 63.9 to 64.5 → 61.8 to 62.7 (64.1 → 62.35);
  seed 3 64.0 to 64.7 → 61.8 to 62.7 (64.2 → 62.15).
- `ab.py`, 41 alternating runs per binary, medians (minimums): seed 1 128.75 → 128.01 (126.90 →
  126.43) ms; seed 3 134.67 → 135.20 (132.45 → 132.43) ms. Before two line cuts that leave the
  logic as is (an `if` around the call, and acquire/release orderings): seed 1 128.57 → 128.08
  and 128.84 → 129.01, seed 2 129.70 → 130.59 ms. A tie.
- S 442 → 449. Gate: 512 pass, 0 fail.

The three levels are now within about 1.5 MB of each other: the end of pass 2 (both edge
arrays and both place arrays, 57.2 MiB, with about 4.5 MiB of process, page tables and the
cgroup's per-CPU charge batches), the forward pass 3, and the queries. A lower M needs all three
to fall, and the places only shrink with a slower build (the transposed build below took 6.5 ms).

## Step 18: 30 fewer lines

S counts code lines after rustfmt, and the Dockerfile's lines too. rustfmt's defaults break a chain,
a call's arguments or a tuple past 60 characters, so most cuts below are forms that stay under
those widths rather than longer lines. Net code lines per change:

- The Dockerfile's build stage runs `cd /src && cargo build` with the context mounted, as track A
  does, instead of setting `WORKDIR` and copying it: 7 → 5. Built with `docker build
  --network=none`, the binary is byte-identical to the local cargo build (ef900b06).
- `for_each_line::<N>` parses a line of N numbers into an array, so the edge lines and the query
  lines share it: -4. The queries are kept as `u32` pairs, so `answer` stores `distance` with no
  binding: -1.
- `split_runs` cuts with `split_off_mut`: -4. `sort`'s prefix sum is a `fold`: -2.
- `map_file` maps at least one byte, so an empty file (an empty queries.txt) needs no branch: the
  mapping of an empty file succeeds and the empty slice never reads it. -3.
- `line_chunks` computes each split point in a closure, from `at - 1` on: -2. Its chunks differ
  from step 17's by at most a line at each split, which only reorders edges inside a row.
- `prefetch` takes an `Option` and `Csr::prefetch` is gone, so `step` makes its two prefetches in
  two lines: -6.
- Smaller ones, -1 each: the two arguments through one closure (track A's form), `start` returns
  the queue's top as `step` does, `unpack` mapped over the row, `unpack` and the row copy declared
  together, `Queue::new` without its `Default::default()` binding, and `ahead`'s word and bit read
  from `from` directly.

`step`, with `push` and `pop` inlined, has the same machine code as step 17's, and pass 1 too but
for one equivalent compare (`cmp 9; ja` for `cmp 10; jae`) in its slow digit loop. Pass 2 has one
more instruction per line: LLVM now lays the heavy-weight test out as a not-taken branch and a jump
instead of a taken branch. Pass 3's prefix sum and the per-query code changed.

Code placement mattered. A first version had `step`'s start 32 bytes past a 64-byte line instead
of 16, with the same machine code, and that cost 0.4 to 0.5% of cycles (`perf stat`, 8 and 10
interleaved rounds, never lower than step 17; `perf record` put all of it in `step`). With every
function aligned to 64 bytes (`-C llvm-args=-align-all-functions=6`, for this control only), step
17's code and this step's both lost 0.4 to 0.5% too. Reading the arguments through one closure moves
`step` back to 16, and that is the version kept.

Local, turbo off, against step 17, seed 1 unless named:

- `perf stat`, 10 interleaved rounds of 5 runs: instructions:u 4,991.5 → 4,996.2 M (+0.09%, pass
  2's extra instruction), cycles:u median 3,056.4 → 3,059.3 M (+0.1%, lower in 3 of the 10 rounds).
- `ab.py`, 41 interleaved runs, medians (minimums): seed 1 127.17 → 127.45 (125.01 → 126.47),
  127.40 → 127.64 (125.52 → 125.41) and 127.69 → 127.42 ms (124.65 → 126.06); seed 3 133.02 →
  133.32 (131.07 → 131.46), 132.59 → 132.72 (131.06 → 130.80) and 133.26 → 133.10 ms (131.56 →
  131.01). In the third round a second copy of step 17's binary ran too: 127.60 and 133.46 ms.
  About +0.07% on average, within the spread of the two copies.
- memory.peak, alternating runs: seed 1 median 62.4 → 62.1 MB (5 runs each), seed 3 62.2 →
  62.35 MB (10 runs each, 61.8 to 63.0 against 61.8 to 63.7).
- S 449 → 419 (Rust 442 → 414, Dockerfile 7 → 5). Gate: 512 pass, 0 fail; `cargo fmt --check`
  and clippy report nothing. An empty queries.txt, a graph with no edges, and files without a
  final LF give the same output as step 17.

## Step 19: graph.txt's pages dropped by the threads that finish first

A phase timeline of step 18 (seed 1, P-cores, medians of 11 to 21 runs, ms from the spawn):
`main` starts at 1.9-2.2; pass 1 ends at 11.5-11.8 (its last thread starts 0.9 ms after the
first); the faulting takes 3.4-3.6, the runs set-up 1.2 (380 page faults), pass 2 12.5, pass 3
3.4 and 3.7; the queries run from 36.7 to 124-125, and the first query thread is done 5 ms
before the last, so the threads idle for 39-42 ms of CPU time at the end (3.5 ms each on
average). Then the output takes 0.15 ms and the exit 2.1-3.9 ms. Kernel time is 4.6% of all
cycles (148 M of 3.21 G).

Most of that exit is graph.txt: a `munmap` of it before the exit took 2.45 ms (1.3 ms after a
reboot, likely from the page cache's pages lying closer together), and the exit then took 1.3
ms. Step 12's not-kept rows dropped those pages from busy threads, in pass 2 or from a 13th
thread during the queries, which moved the cost instead of hiding it. Now each query thread,
once the counter has no query left, drops every `threads`-th 8 MB piece of the mapping
(`madvise(MADV_DONTNEED)`), on a CPU that would otherwise wait for the last query. The last
thread to finish still drops its own twelfth, about 0.1-0.2 ms.

`step`'s machine code is unchanged and starts 16 bytes past a line, as in step 18 (the queries'
`Search` pair is built by an array `map`, which put it there).

- `ab.py`, 41 interleaved runs, medians (minimums): seed 1 126.72 → 125.33 (124.49 → 123.37),
  126.97 → 125.18 (124.32 → 123.50); seed 3 133.03 → 131.74 (130.69 → 129.76), 132.16 → 131.25
  (130.82 → 129.45); seed 2, on a busy machine, 131.81 → 131.57 (126.01 → 125.54) ms. About
  -1.0% on seeds 1 and 3.
- Before a reboot of the machine, a first version (2 MB pieces taken from the query counter, with
  `step` 32 bytes past a line), seed 1: 127.13 → 125.15 ms (125.26 → 123.82).
- memory.peak, 5 alternating runs, seed 1: 61.8-63.7 → 62.1-62.5 MB (medians 62.1 → 62.2).
- S 419 → 422. Gate: 512 pass, 0 fail; `cargo fmt --check` and clippy report nothing. An empty
  queries.txt and a graph with no edges give the same output as step 18.

## Step 20: the build's serial parts

Step 19's timeline has two serial steps on the main thread between the parallel passes: pass
2's runs set-up, 1.1-1.2 ms with 380 page faults, and the heavy edges' two maps, 0.3 ms.

- `split_runs` returned a `Vec` of its runs. Pass 2's set-up collects four of them, 10,200
  slices of 16 bytes each, and at 163 KB each is a fresh `mmap` under step 5's threshold, so
  each costs 40 page faults and an unmap. It now returns an iterator over the runs, cut as they
  are asked for: 380 → 207 faults, and the set-up 1.13 → 0.63 ms (phase marks, 21 runs).
- `Csr::heavy` hashed its (node, other end) keys with std's `SipHash`. It now uses `NodeHash`,
  whose `write` xors each id into its state before the multiply, so a label's one id hashes as
  before (state 0) and a pair mixes both: the heavy step 0.32 → 0.15 ms. `step`'s instructions
  are the same as step 18's; one field offset moved, as `heavy`'s hasher is now zero-sized.
- The build ends 0.64 ms earlier (35.67 → 35.03 ms, medians of 21 runs). `step` starts 16 bytes
  past a line (the two graphs passed as an array again, and the `Search` pair as a tuple).

`ab.py`, 41 interleaved runs, medians (minimums), step 19 → step 20: seed 1 124.63 → 123.92
(122.48 → 122.05) and 125.98 → 124.76 (123.33 → 122.44); seed 3 132.49 → 131.07 (129.41 →
128.89) and 132.71 → 131.83 (129.92 → 129.22); seed 2 127.40 → 126.22 (125.15 → 124.02) ms. About
-0.85%, and about -1.5% against step 18 in the same rounds. memory.peak, alternating runs: seed 1
61.8-64.5 → 61.8-62.7 MB (medians 62.35 → 62.25, 10 runs each), seed 3 61.9-62.7 → 62.0-64.3 MB
(medians 62.55 → 62.7, 6 runs each); both builds show the rare 64 MB runs. S 422 → 424. Gate:
512 pass, 0 fail; `cargo fmt --check` and clippy report nothing.

## After step 20: the graph's structure, measured for two pruning ideas (not built)

The queries take about 89 of step 20's 124 ms (the build ends at 35 ms), so a large gain has to
come from settling fewer nodes. Two ideas need no preprocessing beyond the build, so we measured
how much each could save on seed 1 (a Python script over graph.txt, counting each node's distinct
neighbours with edge direction ignored):

- Neighbours per node: 17 nodes have none, 101,147 have one, 441,141 two, 701,013 three, 444,748
  four and 52,446 five or more.
- **Dead-end trees.** Peeling one-neighbour nodes until none is left leaves a core of 1,622,683
  nodes. The other 117,829 (6.8%), the 17 isolated ones aside, sit in trees that each hang off a single core node, and a search
  never needs to enter a tree that holds neither s nor t. So pruning them saves at most 6.8% of
  the settled nodes, and costs a bit test on every relaxation.
- **Chains.** 476,632 of the core's nodes (29%) have exactly two neighbours. Contracting those
  chains into shortcut edges would settle up to that many fewer nodes. It costs a contraction
  pass in the build, a third edge array, and special cases for an s or t inside a chain.

Both at their ideal settle about 34% fewer nodes, so at most about 30 ms of the 89. We did not build
either: the time left before the freeze would not cover both the build and a check of every edge
case against the hash, and track D is not one of our three scored tracks.

## Tried and reverted (local, turbo off, no run ID)

Each row's baseline ran in the same interleaved session as the experiment, so baselines differ
between rows.

| Experiment | Local result (P-cores, seed 1) | Why it lost |
|---|---|---|
| Prefetch `offsets[next]` at every push, alone | 281 to 285 ms, task-clock 3.16 to 3.18 s, against 289 to 291 ms and 3.26 s | It also fetches nodes that are settled much later, or never |
| The push prefetch together with step 2b's prefetch | 208 to 210 ms, task-clock 2.34 to 2.35 s, against 202 to 207 ms and 2.26 to 2.27 s for step 2b alone | The extra prefetches cost more than they hid |
| That pair, plus the last line of the next node's edge row | 209 to 210 ms, task-clock 2.33 s, against 208 to 210 ms and 2.34 to 2.35 s without it | No gain in elapsed time |
| Step 2b's prefetch placed before the stale-entry check | task-clock 2.27 to 2.28 s against 2.26 s; elapsed 202 to 205 ms against 203 to 208 ms, leaving out one noisy round | A tie, and 3 more lines after rustfmt |
| Prefetch only the closer of the root's two children | 192 / 224 / 194 ms, task-clock 2.10 to 2.12 s, against 189 / 217 / 191 ms and 2.08 to 2.11 s for both | Slower than prefetching both |
| Also prefetch the root's four grandchildren | 191 to 195 ms, task-clock 2.13 to 2.14 s, against 190 to 194 ms and 2.12 s | No gain |
| Transparent huge pages off (`prctl(PR_SET_THP_DISABLE)`) | 227 to 240 ms, task-clock 2.48 s, cycles 5.68 G, peak 125.5 to 129.2 MB, against 211 to 230 ms, 2.35 s, 5.24 G, 124.6 to 127.3 MB | The CSR was already 86 of its 94 MB on huge pages mid-run. Its arrays are fully used, so there is no padding to save, and its random reads miss the TLB more on 4 KB pages |
| `MADV_HUGEPAGE` on the CSR | Not built | Only the unaligned array ends (about 8 MB) are on 4 KB pages. Scaled from the 5.5% that all-4 KB pages cost, that is at most about 0.5% |
| 4-ary heap, child picked by a loop | 202 to 205 ms, task-clock 2.24 s, 6.71 G instructions, against 166 to 168 ms, 1.83 to 1.84 s, 5.69 G | 18% more instructions. The heap tops out at 43 KB and stays in cache, so fewer levels save no misses |
| 4-ary heap, child picked without branches | 212 to 213 ms, task-clock 1.96 to 1.98 s, against 201 to 205 ms and 1.86 to 1.88 s (two rounds, on a busy machine) | The same: std's binary heap is already cheap per level on a heap this small |
| Radix heap, with each bucket's minimum kept on push | 203 to 208 ms, task-clock 2.25 to 2.31 s, cycles 4.92 to 5.07 G, against 170 to 171 ms, 1.85 to 1.87 s, 4.04 to 4.05 G | It can't name the node after next, so step 2c's prefetch is lost, and bucket bookkeeping costs more than a 43 KB binary heap |
| Labels as `u32` distances, with the rare longer ones in a second map (on step 4) | 163 to 170 ms, task-clock 1.81 s, 5.65 G instructions, peak 114 to 116 MB, against 156 to 158 ms, 1.73 s, 5.27 G, 125 to 128 MB | 7% more instructions and 28 more lines to save 11 MB |
| Step 5's rows read in place, but with an NTA hint on the row prefetch, in reverse order, or even edges then odd ones from one load instruction | cycles 4.14 to 4.35 G, 98 to 129M L2 prefetches, against 4.07 G and 113M read in place (counters, one `perf stat -r 3` each) | The extra prefetches stayed |
| Step 5's rows read as 8-byte pairs, or even and odd edges from two load instructions | 55 to 60M L2 prefetches, but 5.85 to 5.93 G instructions and cycles 4.07 to 4.12 G, against 5.45 G and 4.07 G read in place (same session) | The extra instructions ate the gain |
| Step 5's row copied by a `memcpy` of its exact length | cycles 3.97 G, 5.71 G instructions, against 3.91 G and 5.50 G for the 32-byte copy (same session) | A library call per row |
| A 16-byte copy for rows of up to 4 edges | 89M L2 prefetches, cycles 3.99 G, against 55M and 3.91 G for the 32-byte copy (same session) | Rows of 5 to 8 edges still set the prefetchers off |
| Two or four lines parsed at once per thread, interleaved, in both parse passes (on step 5, phase marks) | Two: pass 1 9.5 → 8.5 ms, pass 2 19.0 → 21.5 ms, 157.1 → 158.1 ms before the output. Four: 10.4, 22.6 and 162.6 ms | Pass 2 lost more than pass 1 gained |
| Blocks of 4,096, 8,192, 16,384 or 32,768 nodes instead of 2,048 (on step 5, phase marks) | Runs set-up 1.2 → 0.6 / 0.3 / under 0.2 / under 0.2 ms, pass 2 19.1 → 18.6 / 18.2 / 17.8 / 17.6 ms, but pass 3 6.6 → 7.6 / 14.5 / 12.6 / 15.4 ms, so 157.0 → 156.6 / 162.5 / 161.5 / 165.8 ms before the output | A tie at 4,096, and pass 3 slower beyond. Past 64 KB a block's copy is a fresh `mmap`; with one buffer per thread reused instead, pass 3 took 6.6 / 7.7 / 8.8 / 9.9 ms at 2,048 / 4,096 / 8,192 / 16,384, against 6.8 ms for the copy at 2,048 |
| Step 6's faulting on all 12 threads, or on a third of them | Faulting plus pass 2: 17.1 and 17.2 ms, against 16.4 ms on half (one write per 2 MB). With one write per 4 KB, 17.7 against 17.0 ms | Zeroing is memory traffic, and more threads only contend for it |
| Step 6's faulting for the edge arrays only | 2.8 + 15.9 = 18.6 ms, against 4.3 + 12.4 = 16.7 ms for all four arrays and 18.8 ms for none (one write per 4 KB) | Pass 2's first writes to the place arrays race the same way |
| Faulting inside pass 1, with the arrays sized from the header's edge count | Pass 1 9.5 → 15.2 to 15.9 ms, and 158.6 to 159.0 ms before the output, against 157.6 ms with step 6's separate step and 160.6 ms with neither | The parse slowed by more than the separate step takes |
| Faulting during pass 1 on 6 extra threads, or on the main thread | Pass 1 9.6 → 23.4 ms (fastest 13.5) and 17.6 ms (fastest 13.8) | The 12 parse threads already fill the 12 CPUs |
| Unmapping graph.txt and freeing the place arrays on background threads | 160.5 ms median with both, 160.4 ms without the unmap, 161.8 ms without the background free (5 rounds) | No gain in elapsed time worth the lines |
| A cheaper bounds check in `number` | Pass 1 fastest 9.39 ms, against 9.43 ms | No gain |
| Pass 2's run lists sized up front | Runs set-up 1.28 → 1.10 ms | 0.18 ms is less than the lines cost |
| A statically linked binary (`-C target-feature=+crt-static`) | Start-up 1.0 → 0.73 to 0.77 ms | Not adopted: untested in the Docker build, for about 0.25 ms |
| Relabeling nodes in BFS order, computed during the run (on step 7) | Queries 93.8 → 84.8 ms, but the order took 80 ms and permuting the graph 52 ms. An ideal order from the generator's coordinates, which a submission cannot use, reached 83.3 ms | Even a 10× faster order would only about break even |
| A locality-preserving hash for the label table, on the ideal order above (on step 7) | Queries 110.9 ms against 83.3 ms with the multiplicative hash on the same order | Slower |
| Labels as 32-bit distances with a fallback map (on step 7) | Queries 95.2 against 93.1 ms, 0.9 MB saved | Slower, for little memory |
| A ring of 1,024 or 2,048 buckets instead of 4,096 | Queries 94.9 and 96.2 against 93.1 ms. Re-measured after the line-count pass: 95.9 against 93.5 ms, and 96.2 with `Queue` deriving `Default`, which a 2,048 ring allows (10 lines fewer) | Not from the far heap: at 2,048 only weights 2,047 to 4,095 move there, 0.07% of edges. Cause not found |
| Balancing the two sides by label count, which drops the queue's length counter (3 lines fewer) | Queries 96.6 against 95.0 ms | Slower |
| Linear-probing and epoch-packed label tables instead of `HashMap` | Lost clearly to hashbrown | Its control bytes make failed lookups cheap |
| Exact pruning on `via + other_top >= best`, or checking the other side when a node is settled | Correct, no speedup | |
| Prefetching the 4th or 5th node ahead, or pre-warming the label slots of the next node's neighbours | No gain | |
| Balancing the two sides by pop count instead of queue length | Slower | |
| Label maps emptied after each query once they pass 14,336 entries, or 28,672 (on step 7, with step 9's pass 3 change) | Peak 72.0 to 72.7 MB against 80.9 to 82.6, but elapsed median 148.6 against 134.7 ms, task-clock 1.55 to 1.57 against 1.41 to 1.42 s | 10% slower: the next big search regrows its map from empty, rehashing at every doubling into freshly mapped pages |
| Step 9's pass 3 change without faulting the offsets in first (on step 7) | Peak 80.9 to 85.2 MB against 80.6 to 83.7, elapsed median 137.2 against 134.1 ms | More threads first-touched each fresh huge page |
| Pass 1 parsed with AVX2: one 32-byte compare finds a line's three separators (on step 11, phase marks) | Pass 1 fastest 9.4 to 9.5 → 7.6 to 7.8 ms, pass 2 unchanged, S +24 | 1.3% of T for 5% more lines, a small loss on the score |
| A write prefetch of a run's next slots in pass 2, three placements (on step 11, phase marks) | Pass 2 fastest 15.1 to 15.2 ms against 12.8 ms | Slower |
| `sort`'s counters in a stack array of `BLOCK + 1` instead of a `Vec` per block (on step 11) | Medians 133.30 and 133.63 ms against 133.31 and 133.57 ms (seeds 1 and 2) | No change |
| Each pass 2 thread drops its chunk's pages of graph.txt from the page tables (`madvise(MADV_DONTNEED)`) when it is done, so that the exit has none to unmap (on step 12) | Medians 131.11, 132.27, 131.01 and 131.92 ms against 131.35, 131.97, 130.69 and 132.26 ms (seeds 1, 2, 1, 2) | A tie, for 4 lines. Unmapping them at the exit takes about 1.7 ms on one thread, and the calls cost about as much, likely in the TLB flushes they send to the other threads |
| The same `madvise` on all of graph.txt from a 13th thread during the queries (on step 12) | Medians 132.09, 155.11, 133.53 and 133.61 ms against 131.67, 154.59, 132.94 and 132.81 ms | 0.4 to 0.8 ms slower, likely from the CPU time it takes from the 12 query threads and the TLB flushes it sends them |
| A static-pie build (`+crt-static`, as tracks A and C use), on step 12 | `ab.py`, 30 interleaved runs, median / min 132.21 / 130.34 ms against 132.01 / 130.38 ms; memory.peak, 6 alternating runs, 70.1 to 72.1 against 70.1 to 71.7 MB | No gain: the dynamic loader's quarter of a millisecond is 0.2% of the run |
| Each pass-2 thread drops its chunk's pages of graph.txt (`madvise(MADV_DONTNEED)`, 3 lines), so the exit has fewer to unmap, as track B's step 17 does | `ab.py`, 30 interleaved runs, medians 132.68 → 131.95, 133.93 → 134.01 (seed 2) and 133.41 → 133.53 ms, minimums 0.6 to 1.3 ms lower; kernel cycles 155 to 157 → 163 to 165 M | No gain in the median: 22k page-cache entries take little time to unmap, and the drops interrupt the threads still parsing |
| A node's edge back to the node that labelled it skipped, with that parent carried in the queue entry, and a label's old ring entry unlinked when the label improves (on step 12) | `ab.py`, 31 runs: the skip alone 132.85 → 133.60 ms, both 132.25 → 132.40 ms (medians). Instructions 5.10 → 4.98 G, branch misses 19.0 → 23.6 M, cycles 3.36 G in both | The skip is a branch on the data that mispredicts, and the 450k stale pops of 7.57M (seed 1) that the unlinking removes saved no time |
| Every label table made with room for 32,768 labels (65,536 buckets) from the start, to skip its growth (on a first version of step 13) | `ab.py`, 31 runs, same session: median 130.67 → 135.73 ms | Most queries had run in 32,768 buckets; spread over twice as many, their labels miss the caches more |
| Line cuts that change the machine code, on step 12: `split_runs` through `split_off_mut` (4 lines), `sort`'s prefix sum as a `fold` (3), its copy iterated in the loop header (1) | `.text` differs from step 12's in 4,175 and 573 lines of disassembly with addresses stripped; the third keeps it but draws a clippy warning (`unnecessary_to_owned`) | Not measured: code placement alone moved this track's time by about 1 ms (step 12) |
| A search's first 3,584 labels kept in a table of their own, 4,096 buckets that stay in cache, and only its later labels in the grown table (on step 14; also 1,792 and 7,168) | `ab.py`, 31 runs, same session: medians 171.56, 178.75 and 159.66 ms against 130.22 ms. Branch misses 18.9 → 43.6 M, cycles 3.32 → 4.42 G (3,584) | Once the small table is full, every lookup asks it first, and its answer is a branch on a load that mispredicts |
| The side with the smaller queue top advanced instead of the side with the shorter queue, while both tops are below 2^32 (on step 14) | `ab.py`, 21 runs: median 133.19 → 138.99 ms, min 129.42 → 134.71 ms | Equal radii settle more nodes than equal frontiers on this graph |
| The node hash without its multiply: the id as it is, and its low 7 bits copied to the top for hashbrown's tag (on step 14) | `ab.py`, 31 runs: median 130.96 → 146.39 ms | The tag then repeats the slot's own low bits, so any label whose home is the same slot matches it, and a miss loads a bucket far more often |
| The edge back to the parent left out with no branch: the parent rides in the queue entry, and a compare over the 8 copied edges moves its edge last and drops it (on step 15) | `ab.py`, 31 runs: median 131.53 → 135.38 ms. Branch misses 18.9 → 17.0 M, but instructions 5.15 → 5.20 G and cycles 3.36 → 3.44 G | The parent's label is a cache hit, so its relaxation was cheap; the compare and the wider queue entries cost more |
| Labels as `u16` distances, keys packed to 2-byte alignment so a bucket takes 6 bytes, longer labels behind a `u16::MAX` in the second map (on step 16) | `ab.py`, 31 runs, two rounds: medians 128.02 → 127.34 and 128.47 → 129.36 ms, minimums 126.56 → 125.93 and 125.85 → 126.20. `perf stat` on step 15: cycles -1.2%, instructions +0.6%, L1 misses +1.8%. memory.peak, 6 runs: median 67.89 → 67.62 MB, highest 68.17 → 68.10 | A tie in time, and the label memory it saves sits below the build peak |
| One label table per thread for both sides, `[u32; 2]` per node, so the relaxation reads the other side's label from the same bucket instead of a second lookup (on step 16) | `ab.py`, 31 runs, three rounds: medians 127.38 → 131.05, 128.02 → 130.70, 128.47 → 131.54 ms. `perf stat`: instructions 4.994 → 4.977 G, but cycles 3.063 → 3.156 G and L2 load misses 3.55 → 4.65 M | The joint table holds both searches' nodes in 12-byte buckets, 128 Ki of them for the big queries, and that misses L2 more often than the saved lookup gained |
| The same joint table with `[u16; 2]`, 8-byte buckets (on step 16) | `ab.py`, 31 runs, two rounds: medians 128.02 → 128.64 and 128.47 → 130.07 ms. memory.peak, 6 runs: median 67.89 → 67.75 MB, highest 68.17 → 67.81 | Still slower; the memory sits below the build peak |
| Labels in a dense `u32` array per search, 2^21 slots, reset through a list of touched nodes (on step 15) | `ab.py`, 11 runs: median 131.04 → 243.08 ms | 24 arrays of 8 MiB written in full on first use, about 200 MB of page faults and zeroing |
| `#[inline(always)]` on `Search::step` (on step 15) | `perf stat`, 5 runs: cycles 3.137 → 3.111 G (-0.8%), instructions +0.8% | Below `push`'s gain, which step 16 took |
| The forward CSR built first, then the reverse one by transposing it, so the two place arrays are never live together (on step 15) | memory.peak, 6 runs: median 67.78 → 64.88 MB, highest 67.97 → 65.52. `ab.py`, 31 runs: median 130.61 → 137.06 ms, min 128.61 → 135.52. Build 34.5 → 42.7 ms (forward count 9.1, fault 3.6, scatter 10.9, sort 3.2; reverse count 3.1, fault 1.6, scatter 5.4, sort 3.5) | 2.9 MB saved for 6.5 ms: the queries' own peak, about 64 to 65 MB, then caps the saving, and the second scatter costs more than pass 2 saves |
| Step 17's pass 3 in four rounds, each its own `on_threads`, with the sorted places given back between rounds | Not built in full: one `on_threads` of 12 threads that do nothing takes 172 to 189 µs (average of 20, three runs), so the forward direction's three extra rounds cost about 0.55 ms | Step 17 frees as much with no extra rounds |
| Step 17 with each thread's sorted blocks counted, and every block below 12 × the least count freed (on step 16) | memory.peak, 6 alternating runs: 62.1 to 62.5 against 63.9 to 64.3 MB | The same memory as step 17, which needs no list of block starts |
| `Csr::heavy_weight` folded into `unpack` behind `std::hint::cold_path()`, as `Csr::row` does for wide groups, 3 lines fewer (on step 17's code with step 18's first cuts) | `perf stat`, 8 interleaved rounds: instructions 4,996 → 5,081 M (+1.7%), cycles 3,063 → 3,102 M (+1.4%), higher in all 8 | The hash lookup is laid out in the relaxation loop, where `#[cold]` keeps it out of line |
| Step 19's pieces taken from the query counter by whichever thread is idle, in 2 MB or 8 MB pieces (one more line than step 19) | `ab.py`, 41 runs: 2 MB pieces -0.25%, -1.06%, -0.27% (seeds 1, 3, 2); 8 MB pieces -1.16, -1.12, -0.50, -1.09, -1.64, -1.04, -1.10, -0.83%; the same 8 MB claiming in its rustfmt-compliant form -0.41, +0.04, -0.78, -0.95, -0.94, -0.57, -0.84, -0.78% | No better than step 19's fixed shares, for a line more |
| The build's faulting by `madvise(MADV_POPULATE_WRITE)` on 6 threads, with the runs set-up on the main thread at the same time (on step 18) | Phase marks, 11 runs: faulting and set-up together 4.5 → 10.6 ms, 1,700-2,250 page faults instead of 410 | The populate faulted 4 KB pages, not 2 MB ones |
| No signal stack per thread: SIGSEGV and SIGBUS set to `SIG_IGN` from `.init_array`, before std's runtime setup, so std gives no thread an altstack (on step 18 with step 19) | Phase marks, 11 runs: the build 36.21 → 36.00 ms, the queries' end unchanged | 0.2 ms for about 9 lines |
| The queries on 6 threads instead of 12 (on step 18) | `ab.py`, 11 runs: 125.75 → 157.77 ms | Hyperthreading is worth 1.36× in the queries |
| Pass 3's blocks taken from a shared counter, each behind a `Mutex`, instead of dealt out in turn (on step 20, 4 more lines) | Phase marks, 21 runs: pass 3's end 36.88 → 35.65 ms median, 34.55 → 34.58 fastest, the queries' end 127.74 → 127.46. `ab.py`, 41 runs, median (min): seed 1 124.46 (122.38) → 124.06 (122.76), seed 3 133.28 (129.82) → 136.08 (130.39) ms | The last block ends sooner, but the run does not |
| The build's faulting as 2 MB pieces taken from a shared iterator by 6 threads, instead of 6 fixed shares (on step 20, same line count) | Phase marks, 21 runs: faulting 3.57 → 3.43 ms median, 3.45 → 3.13 fastest, idle 3.20 → 1.87 ms·cpu. With the pass 3 counter above, `ab.py`, 41 runs: seed 1 124.70 (122.99) → 124.28 (122.14), seed 3 130.81 (128.17) → 130.25 (127.90) ms | The zeroing is bound by memory bandwidth, so balance saves little: -0.4% for 4 lines |
| `on_threads` spawning as a tree: each new thread hands half of its items to another new thread (on step 20) | Phase marks, 21 runs: pass 3's last thread starts 28.85 → 32.25 ms, the build ends 35.18 → 38.02, the run 124.31 → 126.89 ms | Each level waits for a new thread to wake up |
| The rows faulted in on the main thread instead of 2 threads, and `on_threads` running its first item on the calling thread (on step 20, 2 more lines) | `ab.py`, 41 runs: seed 1 124.07 (121.96) → 124.00 (122.28), seed 3 130.05 (128.90) → 129.88 (128.21) ms | No gain: thread start-up is not what the phases wait for |
| Each line's separators found at once by a 32-byte compare (LLVM makes it `vpcmpeqb` and `vpmovmskb`), so the next line's start does not wait for the numbers (on step 20) | Phase marks, 21 runs: with the fallback as a `cmov`, pass 1 9.6 → 13.4 ms; as a second loop, pass 1 9.6 → 10.8 and pass 2 12.4 → 16.9 ms | The parse is bound by instruction count (step 12), and the mask is more instructions |
| Two or three queries per thread, advanced one step each in turn, so one query's misses overlap the other's (on step 20) | `ab.py`, 21 runs: 124.77 → 139.64 ms (two), 154.38 ms (three) | Slower |
| Each direction's rows started on a 2 MB boundary inside an allocation 2 MB larger, so all their 2 MB pages can be huge pages (on step 20, 5 more lines) | PEBS: 45% of the 4 KB page walks are the rows prefetch, 19% the edges prefetch. `perf stat`, 3 runs: 4 KB walks 3.7 → 1.1 M, walk cycles 117 → 46 M, cycles 3.211 → 3.183 G. `ab.py`, 41 runs: seed 1 124.42 (122.06) → 123.19 (121.41), seed 3 130.54 (128.61) → 129.79 (128.12) ms. memory.peak, 8 alternating runs, medians: seed 1 62.5 → 63.1, seed 3 62.35 → 63.15 MB. With the edge arrays aligned too: 0.66 M walks, peak about 2 MB higher | Each array's last 2 MB page becomes a huge page as well, so +0.7 MB, which with 5 lines cancels the -0.8% |
| Both directions' rows in one allocation and both edge arrays in another, so that glibc's header of the second array no longer sits inside a 2 MB page, which then cannot be huge (on step 20, 2 more lines) | `perf stat`, 3 runs: 4 KB walks 5.1 → 0.87 M, cycles 3.2445 → 3.2425 G. `ab.py`, 41 runs: seed 1 125.18 (122.68) → 125.58 (123.15), seed 3 130.83 (128.42) → 132.23 (129.21) ms | Fewer walks, no faster |
| glibc's mmap threshold raised to 32 MB once the build is done, so the label tables come from the threads' arenas, whose 2 MB pages can be huge pages (on step 20, 1 more line) | `perf stat`, 3 runs: 4 KB walks 5.2 → 4.2 M, cycles 3.199 → 3.186 G. memory.peak, 4 alternating runs: 62.1 to 62.3 → 66.1 to 68.6 MB | The arenas keep the tables a table outgrew, and the queries then peak 4 to 6 MB above the build |
| `#[inline(always)]` on `Search::step` again, now that `push` is inline (on step 20) | `ab.py`, 41 runs: seed 1 124.37 (122.81) → 126.25 (123.74) ms | Slower |
| Every function aligned to 64 bytes (`-C llvm-args=-align-all-functions=6`), to keep line cuts from moving the hot loops (on step 17 and on step 18) | `perf stat`, 8 interleaved rounds: cycles +16.1 M (step 17's code) and +12.9 M (step 18's), +0.4 to 0.5%, higher in all 8 | `step` then starts on a line, which is slower than 16 bytes past one |
| `sort`'s degree count and prefix sum as `for_each` calls, 4 lines fewer (on step 17) | Same machine code | Pedantic clippy's `needless_for_each` rejects both; the prefix sum went in as a `fold` instead |
