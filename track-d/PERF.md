# Track D: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 72.69 | 893 | 0 |
| 1 | Rust v1: forward and reverse CSR built by a counting sort with atomic adds, and a bidirectional Dijkstra per query on 12 threads (475265e) | run-3d72cc2b20e3 | 0.438 | 141 | 58.2 |
| 2 | Counting sort with no atomics (482dcb3), then a prefetch of the next node's edges (c13e637), two steps deep (599190a) | run-97e259c08579 | 0.1889 | 128.2 | 66.7 |
| 3 | Numbers parsed eight bytes at a time (85568b4) | run-49af01c44df1 | 0.1665 | 125.8 | 68.0 |
| 4 | Heap entries ordered by one packed 128-bit key (542bc46) | run-05e531e8754c | 0.1565 | 125.2 | 68.6 |

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
