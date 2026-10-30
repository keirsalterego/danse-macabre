# Track D: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 72.69 | 893 | 0 |
| 1 | Rust v1: forward and reverse CSR built by a counting sort with atomic adds, and a bidirectional Dijkstra per query on 12 threads (475265e) | run-3d72cc2b20e3 | 0.438 | 141 | 58.2 |
| 2 | Counting sort with no atomics (482dcb3), then a prefetch of the next node's edges (c13e637), two steps deep (599190a) | run-97e259c08579 | 0.1889 | 128.2 | 66.7 |
| 3 | Numbers parsed eight bytes at a time (85568b4) | run-49af01c44df1 | 0.1665 | 125.8 | 68.0 |
| 4 | Heap entries ordered by one packed 128-bit key (542bc46) | run-05e531e8754c | 0.1565 | 125.2 | 68.6 |
| 5 | Edges packed into 4 bytes and read through a 32-byte copy, and glibc's mmap threshold fixed at 64 KiB (680f4ad) | run-74c555d4ba4f | 0.1551 | 83.9 | 70.2 |
| 6 | The build's arrays faulted in before pass 2, one write per 2 MB on half the threads | | | | |
| 7 | Each search's binary heap replaced by a bucket queue: a ring of 4,096 one-unit buckets found through a bitmap, with a small heap for keys past the ring | | | | |

Local numbers are for full seed 1 unless a seed is named. "P-cores" means CPUs 0–11 of an
i7-13620H (its six P-cores and their hyperthreads), with all 12 threads and turbo off. Each figure
is `perf stat -r 5`, and variants were interleaved over three rounds. Peak memory is `memory.peak` of a
systemd-run scope on CPUs 0–11, with the inputs already in the page cache. A single board run is
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

- P-cores: elapsed 394–410 → 289–296 ms, task-clock 4.34–4.45 → 3.25–3.33 s, cycles 10.1–10.4
  → 7.4–7.6 G.
- Build phase on P-cores: about 60 ms after the change (21 + 31 + 8 ms for the three passes).
- Peak memory: 139.7–142.2 → 126.0–129.9 MB, from the narrower offsets. S 217 → 291.

## Step 2b: prefetch the next node's edges (c13e637)

After this change the queries were 80% of the run, and two dependent cache misses dominated each
settled node: its offsets entry, then its edge row, both at random spots in big arrays. The edge
row load alone was 22% of all samples and the offsets read 10%. The node settled next is usually the
one on top of the heap, so `step` now starts loading that node's edges before it relaxes the
current node's, which hides the wait behind that work.

- P-cores: elapsed 289–291 → 202–207 ms, task-clock 3.26 → 2.26–2.27 s, cycles 7.45 → 5.06–5.07
  G.
- Peak memory unchanged. S 291 → 300.

## Step 2c: two steps deep (599190a)

With the edge rows prefetched, the biggest stall became the offsets read for that prefetch (17.5%
of samples). The node after next is usually one of the heap root's two children, so `step` also
prefetches their offsets entries. One step later the top's offsets are already in cache.

- P-cores: elapsed 202–204 → 188–190 ms, task-clock 2.26–2.27 → 2.11 s, cycles 5.06–5.08 → 4.70
  G.
- Seed 2: 204 → 194 ms. Seed 3: 218 → 198 ms.
- Peak memory unchanged. S 300 → 306.

## Step 3: numbers parsed eight bytes at a time (85568b4)

The byte-at-a-time parser was about 16% of all samples. Its loop runs once per digit and exits on
the first non-digit, so the exit branch follows the varying number lengths. The new parser reads 8 bytes
as one `u64` and finds the separator from bit 4, which every digit has and space and LF don't. It
then merges up to 7 digits with three multiplies and no branch. Numbers of 8 or more digits take
the old loop; on seed 1 that is 50 of the 5M weights.

- P-cores: elapsed 193–216 → 167–190 ms, task-clock 2.09–2.12 → 1.82–1.84 s, cycles 4.66–4.71 →
  4.03–4.05 G, instructions 6.59 → 5.69 G.
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

- P-cores: elapsed 165–169 → 157–161 ms, task-clock 1.82–1.84 → 1.73–1.74 s, cycles 4.04–4.06 →
  3.80–3.81 G, instructions 5.69 → 5.27 G.
- The heap's share of samples went from 35.0% to 28.2%.
- Seed 2: 171 → 161 ms. Seed 3: 180 → 165 ms.
- Peak memory unchanged. S 317 → 323.

## Step 5: 4-byte edges and a fixed mmap threshold (680f4ad)

Where step 4's peak went, from `memory.stat` sampled through a seed 1 run: 123.6 of its 127.2 MB
were anonymous memory, and the peak came near the end of the queries. Of that:

- the two CSRs, 2 × (7 MB of offsets + 40 MB of edges) = 94 MB;
- the label maps, about 22 MB: of the 24 searches' maps, 17 had grown to 57,344 entries (1.1 MB)
  and 7 to 28,672;
- 5–10 MB of free heap space, left behind by label maps that grew;
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

- P-cores: elapsed 158–162 → 164–167 ms, task-clock 1.75–1.78 → 1.80–1.82 s, cycles 3.82–3.88 →
  3.97–4.01 G, instructions 5.27 → 5.50 G.
- Peak memory: 126.6–129.2 → 81.7–81.9 MB.
- Seed 2: 159 → 164–166 ms, peak 123.0–123.5 → 81.7–84.2 MB. Seed 3: 164 → 170–171 ms, peak
  123.5–125.5 → 80.1–83.6 MB.
- Knockouts in the same session: without the row copy, 172–177 ms, task-clock 1.90–1.95 s, peak
  81.0–82.3 MB; without the fixed threshold, 164–165 ms, task-clock 1.78–1.79 s, peak 90.6–95.6 MB.
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
as long. In a run that stopped after the build, 0.21–0.24 G of its 0.95 G cycles were in the kernel,
71% of those in `clear_page_erms`, which zeroes new pages. Pass 2 is the first write to the 60 MB of
edge and place arrays, and every thread writes into every block, so all 12 threads fault on each new
2 MB huge page at once. Each of them zeroes a huge page; the kernel maps one and frees the rest.

Now, before pass 2, half the threads each write one value per 2 MB of their own share of the arrays,
so each huge page is faulted in by one thread, bar those that straddle two shares.

- Phase marks (seed 1, 7 runs each, interleaved): pass 2 18.7 → 13.0 ms, plus 3.3 ms for the new
  step, so 18.7 → 16.2 ms in all, and 156.7 → 154.5 ms before the output. A second session: 19.3 →
  16.4 ms, and 158.9 → 155.6 ms.
- P-cores, seed 1: elapsed 164.7–165.8 → 161.2–167.9 ms (medians 165.3 → 163.7), task-clock
  1.80–1.82 → 1.74–1.76 s.
- Seed 2: medians 170.4 → 166.5 ms, task-clock 1.85–1.87 → 1.81–1.82 s. Seed 3: 173.6–175.8 →
  170.1–172.1 ms, task-clock 1.90–1.91 → 1.84–1.85 s.
- With huge pages off (`PR_SET_THP_DISABLE`), the new step takes 0.3 ms and pass 2 faults its 4 KB
  pages as before: 20.8 → 21.2 ms for both, and elapsed medians 173.9 → 173.1 ms. The first
  version wrote once per 4 KB page. That cost 1.6 ms with huge pages off (20.8 → 22.4 ms), and with
  them on it did no better (16.5 against 16.2 ms).
- This laptop runs Debian 13 with huge pages on for all anonymous memory (`always`), and the
  scoring box runs Debian 13 too.
- Peak memory unchanged: seed 1 82.2–83.8 → 83.3–85.4 MB, seed 2 79.8–84.1 → 79.4–85.3 MB.
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
- Each bucket is a LIFO stack of `(node, key)` entries kept in one shared arena with a free list,
  so the arena only grows to the most entries queued at once. A vector per bucket was slower than
  the heap, from its cache misses.
- A key at or past the end of the ring goes to a small binary heap, `far`. Weights go up to
  2^32 - 1 (spec §2), and the near-2^32 edges in every seed take this path. The ring's size is a
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

## Tried and reverted (local, turbo off, no run ID)

Each row's baseline ran in the same interleaved session as the experiment, so baselines differ
between rows.

| Experiment | Local result (P-cores, seed 1) | Why it lost |
|---|---|---|
| Prefetch `offsets[next]` at every push, alone | 281–285 ms, task-clock 3.16–3.18 s, against 289–291 ms and 3.26 s | It also fetches nodes that are settled much later, or never |
| The push prefetch together with step 2b's prefetch | 208–210 ms, task-clock 2.34–2.35 s, against 202–207 ms and 2.26–2.27 s for step 2b alone | The extra prefetches cost more than they hid |
| That pair, plus the last line of the next node's edge row | 209–210 ms, task-clock 2.33 s, against 208–210 ms and 2.34–2.35 s without it | No gain in elapsed time |
| Step 2b's prefetch placed before the stale-entry check | task-clock 2.27–2.28 s against 2.26 s; elapsed 202–205 ms against 203–208 ms, leaving out one noisy round | A tie, and 3 more lines after rustfmt |
| Prefetch only the closer of the root's two children | 192 / 224 / 194 ms, task-clock 2.10–2.12 s, against 189 / 217 / 191 ms and 2.08–2.11 s for both | Slower than prefetching both |
| Also prefetch the root's four grandchildren | 191–195 ms, task-clock 2.13–2.14 s, against 190–194 ms and 2.12 s | No gain |
| Transparent huge pages off (`prctl(PR_SET_THP_DISABLE)`) | 227–240 ms, task-clock 2.48 s, cycles 5.68 G, peak 125.5–129.2 MB, against 211–230 ms, 2.35 s, 5.24 G, 124.6–127.3 MB | The CSR was already 86 of its 94 MB on huge pages mid-run. Its arrays are fully used, so there is no padding to save, and its random reads miss the TLB more on 4 KB pages |
| `MADV_HUGEPAGE` on the CSR | Not built | Only the unaligned array ends (about 8 MB) are on 4 KB pages. Scaled from the 5.5% that all-4 KB pages cost, that is at most about 0.5% |
| 4-ary heap, child picked by a loop | 202–205 ms, task-clock 2.24 s, 6.71 G instructions, against 166–168 ms, 1.83–1.84 s, 5.69 G | 18% more instructions. The heap tops out at 43 KB and stays in cache, so fewer levels save no misses |
| 4-ary heap, child picked without branches | 212–213 ms, task-clock 1.96–1.98 s, against 201–205 ms and 1.86–1.88 s (two rounds, on a busy machine) | The same: std's binary heap is already cheap per level on a heap this small |
| Radix heap, with each bucket's minimum kept on push | 203–208 ms, task-clock 2.25–2.31 s, cycles 4.92–5.07 G, against 170–171 ms, 1.85–1.87 s, 4.04–4.05 G | It can't name the node after next, so step 2c's prefetch is lost, and bucket bookkeeping costs more than a 43 KB binary heap |
| Labels as `u32` distances, with the rare longer ones in a second map (on step 4) | 163–170 ms, task-clock 1.81 s, 5.65 G instructions, peak 114–116 MB, against 156–158 ms, 1.73 s, 5.27 G, 125–128 MB | 7% more instructions and 28 more lines to save 11 MB |
| Step 5's rows read in place, but with an NTA hint on the row prefetch, in reverse order, or even edges then odd ones from one load instruction | cycles 4.14–4.35 G, 98–129M L2 prefetches, against 4.07 G and 113M read in place (counters, one `perf stat -r 3` each) | The extra prefetches stayed |
| Step 5's rows read as 8-byte pairs, or even and odd edges from two load instructions | 55–60M L2 prefetches, but 5.85–5.93 G instructions and cycles 4.07–4.12 G, against 5.45 G and 4.07 G read in place (same session) | The extra instructions ate the gain |
| Step 5's row copied by a `memcpy` of its exact length | cycles 3.97 G, 5.71 G instructions, against 3.91 G and 5.50 G for the 32-byte copy (same session) | A library call per row |
| A 16-byte copy for rows of up to 4 edges | 89M L2 prefetches, cycles 3.99 G, against 55M and 3.91 G for the 32-byte copy (same session) | Rows of 5 to 8 edges still set the prefetchers off |
| Two or four lines parsed at once per thread, interleaved, in both parse passes (on step 5, phase marks) | Two: pass 1 9.5 → 8.5 ms, pass 2 19.0 → 21.5 ms, 157.1 → 158.1 ms before the output. Four: 10.4, 22.6 and 162.6 ms | Pass 2 lost more than pass 1 gained |
| Blocks of 4,096, 8,192, 16,384 or 32,768 nodes instead of 2,048 (on step 5, phase marks) | Runs set-up 1.2 → 0.6 / 0.3 / under 0.2 / under 0.2 ms, pass 2 19.1 → 18.6 / 18.2 / 17.8 / 17.6 ms, but pass 3 6.6 → 7.6 / 14.5 / 12.6 / 15.4 ms, so 157.0 → 156.6 / 162.5 / 161.5 / 165.8 ms before the output | A tie at 4,096, and pass 3 slower beyond. Past 64 KB a block's copy is a fresh `mmap`; with one buffer per thread reused instead, pass 3 took 6.6 / 7.7 / 8.8 / 9.9 ms at 2,048 / 4,096 / 8,192 / 16,384, against 6.8 ms for the copy at 2,048 |
| Step 6's faulting on all 12 threads, or on a third of them | Faulting plus pass 2: 17.1 and 17.2 ms, against 16.4 ms on half (one write per 2 MB). With one write per 4 KB, 17.7 against 17.0 ms | Zeroing is memory traffic, and more threads only contend for it |
| Step 6's faulting for the edge arrays only | 2.8 + 15.9 = 18.6 ms, against 4.3 + 12.4 = 16.7 ms for all four arrays and 18.8 ms for none (one write per 4 KB) | Pass 2's first writes to the place arrays race the same way |
| Faulting inside pass 1, with the arrays sized from the header's edge count | Pass 1 9.5 → 15.2–15.9 ms, and 158.6–159.0 ms before the output, against 157.6 ms with step 6's separate step and 160.6 ms with neither | The parse slowed by more than the separate step takes |
| Faulting during pass 1 on 6 extra threads, or on the main thread | Pass 1 9.6 → 23.4 ms (fastest 13.5) and 17.6 ms (fastest 13.8) | The 12 parse threads already fill the 12 CPUs |
| Unmapping graph.txt and freeing the place arrays on background threads | 160.5 ms median with both, 160.4 ms without the unmap, 161.8 ms without the background free (5 rounds) | No gain in elapsed time worth the lines |
| A cheaper bounds check in `number` | Pass 1 fastest 9.39 ms, against 9.43 ms | No gain |
| Pass 2's run lists sized up front | Runs set-up 1.28 → 1.10 ms | 0.18 ms is less than the lines cost |
| A statically linked binary (`-C target-feature=+crt-static`) | Start-up 1.0 → 0.73–0.77 ms | Not adopted: untested in the Docker build, for about 0.25 ms |
| Relabeling nodes in BFS order, computed during the run (on step 7) | Queries 93.8 → 84.8 ms, but the order took 80 ms and permuting the graph 52 ms. An ideal order from the generator's coordinates, which a submission cannot use, reached 83.3 ms | Even a 10× faster order would only about break even |
| A locality-preserving hash for the label table (on step 7) | Queries 110.9 ms against 93.1 ms | Slower |
| Labels as 32-bit distances with a fallback map (on step 7) | Queries 95.2 against 93.1 ms, 0.9 MB saved | Slower, for little memory |
| A ring of 1,024 or 2,048 buckets instead of 4,096 | Queries 94.9 and 96.2 against 93.1 ms | More keys go to the far heap |
| Linear-probing and epoch-packed label tables instead of `HashMap` | Lost clearly to hashbrown | Its control bytes make failed lookups cheap |
| Exact pruning on `via + other_top >= best`, or checking the other side when a node is settled | Correct, no speedup | |
| Prefetching the 4th or 5th node ahead, or pre-warming the label slots of the next node's neighbours | No gain | |
| Balancing the two sides by pop count instead of queue length | Slower | |
