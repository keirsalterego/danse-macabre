# Track C: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 14.44 | 367 | 0 |
| 1 | Rust v1: a quote-parity pre-pass splits the file for 12 threads, each runs a byte state machine, and one lock-free name table keeps a country bitset per name | run-7bddfb5dd3ec | 0.1204 | 39.3 | 57.4 |
| 2 | Each distinct name copied into one arena entry next to its country bitset | | 0.1191 | | |
| 3 | Transparent huge pages off | | | | |
| 4 | Distinct names counted by the workers, from the atomic OR's return value | | | | |

Local numbers are for full seed 1. "P-cores" means CPUs 0–11 with all 12 threads and turbo off,
which matches the box within about 7%. "E-cores" means CPUs 12–15. They have no hyperthreads, so
they overstate any change that hides memory stalls (see the batching experiment below).

## Step 1: v1

The file is cut into 12 chunks. A parallel pre-pass takes the parity of each chunk's quote bytes,
and a prefix XOR over the parities tells each cut whether it sits inside a quoted note, so every
thread finds its first record without parsing the chunks before it. Each thread then walks its
records byte by byte, looks up the region in an array indexed by region id, and sums into its own
per-country totals. Distinct names go through one lock-free hash table shared by all threads, with
one bit per country for each name.

Locally on P-cores: 135 ms wall, 1.18 s user CPU. On E-cores: 0.416 s, 2.56 G cycles.

## Step 2: name-table entries in one arena

Each distinct name's bytes are copied once into a shared arena entry that also holds its
per-country bitset. Before, a record compared its name against the name's first occurrence in the
200 MB file, a near-certain DRAM miss, then read a separate span table and bitset. Now a record
touches only its hash slot and one entry, where the compare and the atomic OR share a cache line.

- E-cores, `perf stat -r 5`: cycles:u 2.56 → 1.97 G, task-clock 1475 → 1145 ms, elapsed
  0.416 → 0.326 s, instructions:u 2.54 → 2.66 G. S 397 → 412.
- P-cores: about 15% less wall time.
- Board: 120.4 → 119.1 ms, one run each.

## Step 3: transparent huge pages off

The name table's arena grows at one front that all 12 threads write. With THP set to "always",
every thread that faults on a fresh 2 MB region of it before the first fault finishes gets a
2 MB page of its own: the kernel allocates, charges and zeroes the page before it takes the
page-table lock and finds the region already mapped. The losers free their pages, but memory.peak
has already counted them. `prctl(PR_SET_THP_DISABLE)` at startup makes every fault a 4 KB one.

THP is "always" on this machine, and the board's 39.3 MB for step 1 is the local peak with THP on
(37.5 MiB), so the box is likely the same. To find the extra memory, an instrumented copy read the
cgroup's memory.current and memory.stat at each phase boundary on P-cores. When the workers
finished, memory.current was 14.8 MiB (anon 12.7, 6.0 of it huge pages), yet memory.peak was
37.1–37.8 MiB, so 22 MiB existed only for moments inside the worker phase. `/proc/self/smaps` put
the 6 MB of huge pages in the mapping that holds the slots and the arena. On 4 E-cores at most 4
threads fault at once, so the peak there is only about 18 MiB, which is why the first measurements
missed it.

P-cores, three interleaved rounds:

| | step 2 | step 3 |
|---|---|---|
| memory.peak, user scope (MiB) | 37.5 / 37.5 / 37.9 | 15.3 / 15.6 / 15.1 |
| memory.peak, container with the harness's flags (MiB) | 38.0 / 37.3 / 37.9 | 14.7 / 15.0 / 14.7 |
| task-clock, `perf stat -r 10` (ms) | 1186 / 1172 / 1158 | 1202 / 1186 / 1202 |
| elapsed (ms) | 123.3 / 122.0 / 124.4 | 124.7 / 122.8 / 124.5 |

That should read about 16 MB on the board. The 4 KB pages cost about 2% more CPU time and 0.6%
more wall time. S 412 → 415.

What remains, from the same instrumented copy with THP off: 4.0 MiB of hash slots, 5.2 MiB of
arena in use (of 18 MiB reserved) and 1.9 MiB of region array, 11.4 MiB of anonymous memory in
all. In the container the highest sample was 11.5 MiB anon plus 1.1 MiB of kernel memory, 0.6 MiB
of it page tables for the 200 MB input mapping.

## Step 4: distinct names counted by the workers

The distinct-name counts used to come from a walk over every arena entry after the workers had
joined, 9 ms on one thread. Now the atomic OR that sets a name's country bit returns the word it
replaced, so the one record that sees the bit go from 0 to 1, in whichever thread, counts the name
for its country. Each bit flips exactly once, so the per-thread counts always sum to the same
totals, and the walk is gone.

P-cores, `perf stat -r 10`, three interleaved rounds:

| | step 3 | step 4 |
|---|---|---|
| task-clock (ms) | 1200 / 1198 / 1198 | 1139 / 1147 / 1169 |
| elapsed (ms) | 123.9 / 124.0 / 124.0 | 111.5 / 113.1 / 114.2 |

S 415 → 402, and memory.peak is unchanged at 14.8–15.4 MiB. Because the old value is now used,
the OR compiles to a `lock cmpxchg` loop instead of `lock or`. LLVM can emit `lock bts` for this
pattern, but only if the optimizer has not already rewritten the bit test, which it had here. The
loop did not show in task-clock.

## Experiments that did not stay

- **Batched table lookups** (local only): each thread parsed 32 records, then loaded every
  record's region entry and hash slot, then found every name's entry, so the cache misses within
  a batch overlap instead of following one another.
  - E-cores, three alternating runs: cycles:u 1.95–2.00 → 1.70–1.73 G, task-clock
    1129–1163 → 987–1010 ms, elapsed 0.324–0.334 → 0.292–0.297 s, instructions:u
    2.66 → 2.93 G. S 412 → 448.
  - P-cores, `perf stat -r 10`, interleaved: task-clock 1073/1067/1075 → 1089/1076/1086 ms,
    elapsed 125/115/128 → 119/121/123 ms. No gain: on a P-core the hyperthread sibling already
    runs while one thread waits on a miss. Reverted.
  - Variants on E-cores: separate intern and OR steps 1.711–1.762 G (the fused form was equal or
    slightly faster), explicit `prefetcht0` 1.745–1.777 G, and batches of 16 or 64 within noise
    of 32.

## Profile notes

- Knockouts on v1 (E-cores, cycles:u, a noisier run with v1 at 3.08 G): no name table 1.02 G, no
  name compare 2.16 G, no bitset OR 3.19 G (no gain), no region lookup 2.63 G. The name table
  cost about two thirds of all cycles, which is what step 2 went after.
- Knockouts after step 2 (E-cores, cycles:u): full 1.97 G, no name table 1.06 G, no region
  lookup 1.72 G, scan only 1.00 G.
- Phase timeline after step 2 (P-cores, 12 threads, instrumented copy): regions parse 0–11.0 ms
  on one thread; quote pre-pass 11.5–17.1 ms, about 5.5 ms per thread; workers 17.9 ms to about
  104 ms, 85.1–86.8 ms each, so the chunks are balanced; merge about 1 ms; distinct counting
  105.3–114.2 ms on one thread; format and write 0.6 ms. About 20 ms of the 115 ms runs on one
  thread.
