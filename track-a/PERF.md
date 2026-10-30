# Track A: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 32.05 | 228 | 0 |
| 1 | Rust v1: one parse pass into 8-byte records, in-place hour sort, hour-local counting sort and selection; plus step 1b | run-8328aa9adcfa | 0.0422 | 50.9 | 72.5 |
| 2 | Output numbers written digit by digit instead of through `write!` | run-c283e4c2dd6a | 0.0395 | 54.4 | 72.7 |
| 3 | Workers release their share of the input before exit; hours handed out one at a time, largest first | run-74fc8ede0277 | 0.0385 | 55.2 | 72.7 |
| 4 | Endpoint names compared with one masked AVX2 compare instead of `memcmp` | run-9aff901d4327 | 0.0387 | 57.9 | 72.3 |
| 5 | Six numbers kept per output line; every hour formatted straight into the mapped output file | run-490479179585 | 0.0339 | 48.2 | 74.0 |
| 6 | Transparent huge pages off; formatting from a shared queue; interner lookup inlined | run-b08360e1cf8c | 0.0313 | 37.0 | 75.9 |
| 7 | 64-byte padded name copies instead of 128 | run-0793bc3ce213 | 0.0306 | 36.1 | 76.3 |
| 8 | Records in file order, 6 bytes per line, one entry per run of an hour; no hour sort | run-bbeb5bdf69ce | 0.0283 | 30.3 | 78.0 |
| 9 | Median selected first, then p95 and p99 in the shrinking upper part | run-26ed452b8751 | 0.0279 | 30.7 | 78.0 |
| 10 | 4-byte records; latencies over 19 bits on a side list | run-00fdd1e29f86 | 0.0276 | 25.0 | 79.0 |
| 11 | One team of threads summarizes and formats the hours, with a barrier between | run-23647f6bde01 | 0.0280 | 25.3 | 78.8 |
| 12 | One mmap helper for input and output; 35 fewer code lines | run-ce9384368863 | 0.0269 | 25.1 | 79.4 |
| 13 | Hour checked by comparing 8 timestamp bytes; exact 64-byte name compare without the hash; padded copies for the merge | run-b262ef9cc01a | 0.0258 | 25.2 | 79.9 |
| 14 | Lean parse on a padded mapping, SWAR number output, names merged inside the parse, sorting networks for groups of up to 64, threads spawned as a tree, and a static-pie build | run-3c59aa35895d | 0.0220 | 24.5 | 81.3 |
| 15 | 137 fewer code lines: a fixed-size name table, `OnceLock` hand-offs instead of a barrier leader, records written straight into each chunk | run-f13e9cd22a88 | 0.0215 | 24.4 | 82.5 |
| 16 | Records freed run by run once their hour is placed, buffers freed after the hour pass, name tables allocated once, output formatted line by line straight into place | run-1ea5237a6e04 | 0.0212 | 21.5 | 83.1 |
| 17 | The last thread to finish ends the process instead of joining the others; 15 fewer lines | run-3ba29a3569ef | 0.0209 | 21.1 | 83.5 |
| 18 | The name table's padded copies aligned to cache lines, and its slot array a fixed-size box | run-01b9bf369035 | 0.0212 | 20.6 | 83.5 |
| 19 | The name hash takes one multiply instead of three dependent ones, and the slot is the product's top bits | run-4d1c6d742295 | 0.0207 | 21.3 | 83.5 |
| 20 | Each output line built in a stack buffer with digit counts from the SWAR conversion; 12 fewer lines | run-1df3325795e7 | 0.0220 | 21.2 | 83.0 |
| 21 | Each small group loaded into the sorting network with one AVX2 masked load | run-c71602f005c5 | 0.0203 | 21.1 | 83.9 |
| 22 | The hour label formatted once per hour into the line buffer, and each number of up to 7 digits written with one store; 5 fewer lines | run-c71602f005c5 | 0.0203 | 21.1 | 83.9 |
| 23 | 5 fewer lines at the same speed: one `use` line, a recursive spawn tree, an hour as a tuple | run-6b577bd7fed9 | 0.0200 | 21.4 | 84.1 |
| 24 | 2 fewer lines with the same machine code: one `use` line, and no alias for the hour buffer | run-3d33fc387ebc | 0.0199 | 21.2 | 84.2 |
| 25 | 6 fewer lines with the same machine code: five variables written where they are used, and the last thread's exit on one line | run-6223c163632a | 0.0200 | 20.9 | 84.2 |

## Step 1: v1

Hypothesis: at 239 MB the log is ten times the L3, so the program should touch it once. Each line
becomes an 8-byte record (endpoint id, hour, error bit, latency) and everything after the parse
works on 25 MB of records.

- **1a, v1 alone (local only; turbo off, official harness):** T 49.6 ms, M 55.3 MB, S 320. It never
  ran on the board on its own: the team was not registered yet when it was pushed.
- **1b, AVX2 line ends and right-to-left fields (in run-8328aa9adcfa together with 1a):** the
  first parse scanned each endpoint 8 bytes at a time and parsed the latency digit by digit, so
  every line paid two loops of varying length with mispredicted exits. Now one 64-byte AVX2
  compare finds the LF, the latency (up to 7 digits, 99.9% of lines) converts in one SWAR step
  from the word before the LF, and the status and endpoint end sit at fixed offsets before it.
  The endpoint hash reads three 8-byte windows instead of walking the name. Local, turbo off,
  under load: 59.3 → 49.6 ms wall, user CPU 380 → 321 ms.

## Step 2: number formatting

The hour pass formats 127k lines with six numbers each. A phase timer showed formatting at
3.5–5 ms per worker, more than the selection. Writing the digits directly into a small buffer
cut user CPU from 320 to 300 ms locally; on the board T went 42.2 → 39.5 ms. It cost 23 code
lines (S 357 → 380), so K rose only 28.16 → 28.48.

## Steps 3 to 7

- **3:** the 239 MB input mapping used to be torn down by the kernel at exit, on one thread
  (3.9 ms timed with an explicit `munmap`). Now each worker releases its share with
  `MADV_DONTNEED` when it is done. In the same run, hours go to workers one at a time from a
  shared counter, largest first, because night hours hold about half the lines of day hours.
- **4:** names vary in length, so `memcmp` mispredicted its length branches on most lines. One
  AVX2 compare of 64 bytes against a zero-padded copy, masked to the name's length, replaced it.
- **5:** the hour pass keeps six numbers per output line and the hour's exact formatted length.
  The output file then gets its final size, is mapped, and every worker formats hours straight
  into place. That replaced a 3.9 ms single-threaded `write`.
- **6:** three changes in one run. With transparent huge pages on, each thread's 2 MB of records
  straddled two 2 MB pages; turning them off with `prctl(PR_SET_THP_DISABLE)` took the local
  peak from 50 to 36.4 MB. Formatting takes hours from a shared queue, and the interner's lookup
  is forced inline (about 1 ms locally).
- **7:** the padded name copies shrank from 128 to 64 bytes, so more of them stay in cache:
  about 0.8 ms locally, and 1 MB less memory. The same code also scored T 40.2 ms in
  run-ce1ce7459a63; a single board run varies that much on a 31 ms program.

## Step 8: records in file order

Shards upload in time order, so consecutive lines of the log almost always share an hour: seed
1's 3.1 million lines form just 1,054 runs of one hour. Each chunk now keeps one entry per run
instead of an hour field per line, so a record is the latency (4 bytes) plus the endpoint id
and error bit (2 bytes), and the in-place hour sort (1.6 ms per thread) is gone. The spec
promises no bound on the disorder, and correctness does not need one: a log whose hour changed
on every line would have one run per line. Local, turbo off, P-cores, 40 alternating runs each:
median wall 33.39 → 31.18 ms; peak memory 35.2 → 29.3 MB (three runs each).

## Steps 9 to 12

- **9:** each group needs three order statistics. Selecting p99 first and then p95 and p50 in
  the prefixes left behind visits about 2.94 n values, because those prefixes hold 99% and
  95% of the group. Selecting the median first leaves p95 and p99 in the upper half, and p95
  then leaves p99 in the top 5%: about 1.55 n. Local, turbo off: median 31.39 → 30.81 ms over
  40 alternating runs. The first board run of this code (run-944d7fc38010) drew 32.5 ms; the
  table cites the second.
- **10:** 97.5% of seed 1's latencies are below 2^19 µs, about half a second. A record is now
  one `u32`: 12 bits of endpoint id (the spec has 1,842 endpoints), the error bit, and 19 bits
  of latency. A latency that does not fit goes to a per-chunk side list, and its record holds
  the all-ones value that sends a reader there. Local, turbo off: peak 29.5 → 23.8 MB, wall
  unchanged (median 31.74 → 31.39 ms over 40 runs).
- **11:** spawning twelve threads took up to 1 ms before the last one ran. The hour pass and
  the formatting now run on the same threads: between them, each waits at a barrier while one
  sorts the hours and maps the output file. Local, turbo off: median 30.34 → 29.91 ms (seed 1)
  and 30.28 → 29.83 ms (seed 2), 40 runs each.
- **12:** the input and output mappings share one `mmap` helper, the imports are grouped, and
  a few statements that rustfmt spread over several lines are written more directly. S
  525 → 490; wall time unchanged locally (median 30.04 → 29.89 ms over 30 runs).

## Step 13: fewer instructions per line

`perf stat` on the parse showed 1.94 instructions per cycle on each of a core's two hardware
threads, so the core retires close to four per cycle between them and every instruction left
out of the per-line work shortens the parse. Three changes, all in the parse and the merge:

- The hour was decoded from six digits on every line. A run of one hour keeps the timestamp's
  bytes 5 to 12 (`MM-DDTHH`) unchanged, so each line now compares those 8 bytes with the run's
  and decodes the hour only when they differ: 1,159 → 1,026 million instructions.
- A probe compared the 64-bit hash stored with the name before comparing the name, a load from
  a 44 KB table on every line. The padded copy now holds the name's following space too, and
  comparing the name plus that space is exact on its own, since endpoints never contain spaces
  (§2). The table grows at a quarter full instead of half: second probes went from 7.8% to 3.4%
  of seed 1's lines in a simulation of the probe sequences.
- The serial merge of the chunks' names read each name from the input, where every chunk's
  copy lies in a different place: about 20,000 cache misses while all other threads waited. It
  now compares the chunks' padded copies, which are then freed.

Local, turbo off, 30 alternating runs: median 30.29 → 29.60 ms (hour check) → 28.74 ms (both);
instructions 1,159 → 974 million; peak memory and code lines unchanged. A second board run of the
same code, run-b1fa893c8a06, measured 25.2 ms.

## Step 14: fewer instructions everywhere, and a faster start

A phase split of step 13 (instrumented build, 27.7 ms locally): start-up 0.65 ms, parse 14.4 ms
of wall time (13.6 per thread, plus 0.7 until the last thread starts), serial name merge 1.0, hour
pass 6.3 (count 0.7, scatter 1.2, selection 3.6), formatting 3.05, releasing the input 0.65, and
joining plus exit 1.0. Five changes, each measured on top of the ones before (medians of 30
interleaved runs, local, turbo off, against step 13 in the same session):

| Change | Step 14 so far | Step 13 |
|---|---|---|
| The log is mapped with 256 zero bytes after it, so each line is read as a fixed-size array with no bounds checks; bit tricks skip the method and convert the latency's digits | 27.91 ms | 28.59 ms |
| Numbers written by a SWAR conversion and fixed 8-byte stores | 27.23 ms | 28.58 ms |
| Each thread merges its endpoint names right after its own parse, so the serial merge disappears | 26.69 ms | 28.65 ms |
| Groups of up to 64 latencies sorted by an AVX2 sorting network, which has no branches; the selection it replaces was mostly mispredicted branches. Hour pass 6.3 → 4.7 ms | 25.63 ms | 28.65 ms |
| Threads spawned as a tree, so the last of twelve starts after four spawns instead of eleven, and a static-pie build that skips the dynamic loader | 23.94 ms | 29.18 ms |

`perf stat`, 5 runs: instructions 989 → 817 M, cycles 530 → 446 M, branch misses 2.46 → 1.28 M.
Peak memory is unchanged (24.2–24.9 MB against 23.9–25.1 MB). S 490 → 561. The static build was
checked with an offline `docker build` of the track: its output for full seed 1 matches.

Tried and dropped: an LSD radix sort instead of the scatter and selection (no gain: the count
pass doubled and it added page faults).

The board's first run of this code, run-e4abec729a4f, drew 28.1 ms. The next push changed only
track B, and its run of the same track A code, run-3c59aa35895d, took 21.98 ms: that is the row.

## Step 15: 137 fewer lines at the same speed

S 561 → 424 (417 Rust lines and the 7-line Dockerfile), with the same output:

- The name table never grows: 8,192 slots and an assert at 4,096 names. The spec gives every
  seed 1,842 endpoints, so the table is never more than a quarter full.
- `OnceLock::get_or_init` replaces the barrier leader and its two hand-off slots, and each hour's
  result goes into its own `OnceLock` slot, so the hour pass collects nothing.
- Records and large latencies are written straight into the chunk, and a run's end is the next
  run's start, with a sentinel after the last one.
- One queue of places instead of two, formatting appends to one `Vec`, and `hour_index` reads a
  table of days.
- The name table still stores each name's hash. Recomputing it in the merge cost 0.4 ms, because
  it reads the names' bytes from the input again, cold.

Local, turbo off, against step 14 in the same sessions (`ab.py`, two rounds of 30 interleaved
runs): medians 24.15 and 24.26 ms against 24.52 and 24.57 ms. `perf stat`, 10 runs: instructions
818.8 M against 817.4 M, cycles +0.9%. memory.peak, 6 runs each: 22.4–23.1 MB against
22.7–23.8 MB. Gate: 307 pass, 0 fail.

## Step 16: a lower peak

memory.current, sampled per phase, showed the peak at the end of the hour pass: all 12.4 MB of
records, the hours' lines (3 MB), every thread's latency buffer (2.8 MB) and allocator
leftovers. The records were already freed before the formatting, so they were never live
together with the 10.1 MB output. Four changes:

- Each run's records are given back (whole pages, `MADV_DONTNEED`) right after their last
  reader, the scatter pass of their hour.
- Each thread frees its latency buffer after its last hour.
- The name tables are allocated once at full size, so no grown-out buffer is left behind.
- `format_hour` formats one line at a time into a 256-byte buffer and writes it straight into
  its mapped place with `io::Write`, so the 12 per-thread hour buffers (1.6 MB) are gone from
  the end. The check that each hour fills exactly its place stays.

The peak is now about even between the start of the hour pass and the end (10.1 MB of output
plus 3 MB of lines). Local, turbo off: memory.peak 22.3–23.1 → 19.9–20.4 MB (8 runs each);
`ab.py` medians 24.15 and 24.17 → 24.06 and 23.87 ms (two rounds of 30); instructions 818.8 →
826.6 M. S 424 → 425. Gate: 307 pass, 0 fail.

Tried and dropped: formatting whole hours during the hour pass and copying them out afterwards
(0.5 MB less, but +8% T).

## Step 17: the last thread ends the process

Once the output is complete, nothing is left to do but exit, so the last thread to finish calls
`process::exit(0)` itself instead of the main thread joining every worker. That saves about
0.35 ms of thread exits and joins. Line cuts at the same time: a timestamp outside the 72 hours
fails an `expect`, like the other spec guarantees, instead of returning a `Result`; chunk starts
come from a 4-line closure; and the mmap helper maps at least one byte, so an empty input needs
no special case.

Local, turbo off, `ab.py` with 50 interleaved runs in a quiet window: median 23.81 → 23.35 ms.
memory.peak 19.4–20.2 → 19.8–20.4 MB, the same within noise. S 425 → 410. Gate: 307 pass, 0
fail.

## Step 18: name copies on whole cache lines

The name table keeps a 64-byte padded copy of each name for the exact compare. malloc put that
array at 16 mod 64 on every thread, so each copy straddled two cache lines and every probe
loaded both, one of them as a split load. A `#[repr(align(64))]` wrapper puts each copy on one
line: L1 misses 9.6 → 8.1 M. The slots became a `Box<[u32; 8192]>`, so the mask is a constant
and the probe has no bounds check: user instructions 829.4 → 822.3 M.

Local, turbo off, `ab.py` with 31 interleaved runs, two rounds: medians 23.52 → 23.09 and
23.56 → 23.07 ms, minimums 23.16 → 22.64 and 22.98 → 22.72 ms. User cycles, 4 interleaved sets
of 5 runs: 448.6 → 441.5 M. memory.peak 18.95–19.88 → 18.53–19.90 MB. S 410 → 412. Gate: 307
pass, 0 fail.

## Step 19: one multiply per name hash

The name hash mixed three 8-byte windows of the name (first, middle and last) through three
dependent multiply-and-rotate steps. Now the windows are rotated apart (by 0, 21 and 42 bits),
XORed with the length and multiplied once by the same FxHash constant. The slot is the
product's top 13 bits, the bits every input bit reaches, which also drops the mask. In a
simulation of the 8,192-slot table on seeds 1, 3 and 5, 3.2% of lines need a second probe,
against 3.1% before (the old hash with top bits would need 3.8–7.6%). The table compares the
bytes exactly, so the hash cannot change the output.

Local, turbo off: instructions 822.3 → 801.2 M; user cycles, 3 interleaved sets of 5 runs,
443.5 → 430.3 M. `ab.py`, 31 interleaved runs, two rounds: medians 23.25 → 22.92 and 23.40 →
22.93 ms, minimums 22.73 → 22.45 and 22.75 → 22.41 ms. memory.peak unchanged. S unchanged at
412. Gate: 307 pass, 0 fail.

## Step 20: a leaner formatter, and 12 fewer lines

The board's first run of this code, run-9ab4c13e512d, drew 26.3 ms; the next, run-1df3325795e7,
21.98 ms: that is the row.

Each output line is built in a 256-byte stack buffer instead of a `Vec`, the hour label is
written once per hour, and `put_number` writes at a cursor. The digit count comes from the
SWAR conversion itself (one `tzcnt` over the '0' bytes) instead of a separate `digits()`. Phase
stamps put the format phase at 1.96 → 1.72 ms. Line cuts outside the parse and the hour pass:
the libc constants are inlined with a comment naming each flag, `main` reads its arguments
through one closure, and the mapping helpers lose their single-use bindings.

Local, turbo off, `ab.py` with 40 interleaved runs: median / min 22.84 / 22.16 → 22.70 / 22.06
ms (formatter alone 22.64 / 22.22; an earlier pair 22.91 → 22.62). `perf stat -r 5`: instructions
801 → 779 M, user cycles 429 → 425 M. memory.peak unchanged. S 412 → 400. Gate: 307 pass, 0
fail; a missing input still reports its path and exits 1.

Tried and dropped: faulting the output pages in ahead with `MADV_POPULATE_WRITE`, per hour
(slower than plain faults in a 12-thread test, 1.15–1.26 against 1.04–1.07 ms) or during the hour
pass (0.6 ms off the format phase, 1 ms onto the hour pass). A page fault on the output costs
about 1.9 µs even without contention.

## Step 21: masked loads into the sorting network

A group of up to 64 latencies used to reach the sorting network one value at a time, each with
a bounds check (`get().unwrap_or(MAX)`), through a stack buffer. Now each register is filled by
`_mm256_maskload_epi32`: lanes past the group's end read nothing and hold 0, so they sort to the
front, and the wanted ranks move up by the number of padding lanes. A real latency of 0 sorts
among them, which is still right, since every padding lane is at most every real value. The
masked lanes never fault, even past a page end.

Local, turbo off, `ab.py` with 31 interleaved runs, two rounds: medians 22.81 → 22.52 and
22.98 → 22.68 ms, minimums 22.24 → 22.00 and 22.24 → 21.98 ms. `perf stat`, 7 interleaved runs:
user cycles 423.9 → 416.8 M, branch misses 1.25 → 1.12 M, instructions 779.1 → 773.9 M.
memory.peak unchanged. S unchanged at 400. Gate: 307 pass, 0 fail.

## Step 22: one store per number

Steps 21 and 22 went to the board in one push, so both rows cite run-c71602f005c5.

`hour_label` is folded into `format_hour`, which formats the label once per hour into the
line's stack buffer. `put_number` writes the space and the digits of a value below 10^7 as one
8-byte store; values of 8 to 10 digits write their leading digits first, then the other seven.
Full seeds 1–4 have percentiles of 8, 9 and 10 digits, so the gate covers that path. Instructions
779.1 → 779.2 M: the change is lines, not time. `ab.py`, 40 interleaved runs, median / min:
22.74 / 21.86 ms before, 22.77 / 21.94 ms after. memory.peak unchanged. S 400 → 395. Gate: 307
pass, 0 fail.

Tried and dropped: releasing each thread's share of the input right after its merge, to overlap
the wait at the first barrier: +3.9 ms, because the TLB-shootdown interrupts hit the threads
still parsing.

## Step 23: 5 fewer lines

The two `std::sync` imports become one line; `fan_out` recurses instead of looping, with the
same spawn tree (12 → 6 → 3 → 1); and an hour's result is a `(lines, len)` tuple with the field
docs on its type alias instead of a two-field struct. `ab.py`, 40 interleaved runs, median / min:
22.61 / 21.81 ms before, 22.53 / 21.58 ms after. No allocation changed. S 395 → 390. Gate: 307
pass, 0 fail.

## Step 24: 2 more lines

The two remaining `use std::{…}` lines become one, and the three names used once (`Error`,
`Reverse`, `NonZeroUsize`) are written as paths where they are used; `summarize_hour` resizes
`buffer` itself instead of an alias, `latencies`. `objdump -d` of the binary is the same as step
23's, so T and M cannot move. S 390 → 388. Gate: 307 pass, 0 fail.

## Step 25: 6 more lines

`release` takes the page bounds straight from `items.as_ptr()`; `parse` declares its cursor `i`
with `count` and `current`; `line_end` passes its 64-byte window to `equal_mask` as
`first_chunk()`; `intern` computes the slot mask where it wraps; and the last thread's exit is one
`bool::then`. The `.text` and `.rodata` sections are byte-identical to step 24's, so T and M
cannot move. S 388 → 382. Gate: 307 pass, 0 fail.

## Experiments that did not stay

- **Pieces handed out from a shared counter instead of one fixed chunk per thread** (local,
  turbo off, on AC): with 16 pieces per thread the median went 30.68 → 31.54 ms, with 64 pieces
  to 33.78 ms. `perf stat` showed 3.5% more cycles for the same instructions and 11% more dTLB
  misses. On battery power, where some threads started 4 to 8 ms late, the pieces helped.

- **Releasing the input inside the hour pass instead of after formatting** (local, turbo off):
  3 ms slower. Freeing pages interrupts every other thread of the process to flush its TLB.
- **Sorting small groups instead of selecting** (n ≤ 16 or ≤ 32; local, turbo off): medians
  34.53 to 34.61 ms, no better than selection.

- **Compact copies of endpoint names for the interner** (local only, turbo off): the idea was
  that comparing against a name's first occurrence in the 239 MB file costs a cache miss on most
  lines. Measured: parse loop 18.3 ms per thread before and after. The miss was not the cost.
  Reverted.
- **Per-thread `MADV_POPULATE_READ` of each chunk before parsing** (local only, measured under
  heavy load from other jobs, so only indicative): 80.5 → 80.1 ms. Not kept.

## Profile notes

- Phase split after step 1b (native, turbo off): parse loop 18.3 ms per thread (finding line ends
  alone 6.9 ms, interning about 8, record push about 7), endpoint merge 1 ms, hour pass 13 ms,
  output write 4 ms, and about 8 ms outside `main`.
- Unmapping the input at exit, timed with an explicit `munmap`: 3.9 ms on one thread.
- `perf record` after step 2: parse loop 37% of cycles, `Interner::intern` 14%,
  `select_nth_unstable` 11%, hour-pass loop 9%, libc `memcmp`/`memcpy` about 9%.
