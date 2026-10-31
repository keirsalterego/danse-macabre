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
| 26 | 3 fewer lines with the same machine code: `digits` a closure in its one caller, and a group's length read where it is used | run-b3536f948213 | 0.0200 | 21.2 | 84.2 |
| 27 | The sorting network's stages within one register take each lane's partner from the register itself, not from the stage's copy | run-ccf7ded5ed1d | 0.0199 | 21.0 | 84.3 |
| 28 | 3 fewer lines with the same machine code: the run pairs taken with `array_windows`, and two lint exceptions moved to `Cargo.toml` | run-ccf7ded5ed1d | 0.0199 | 21.0 | 84.3 |
| 29 | 1 fewer line with the same machine code: the network's compare-exchange blends its minimum and maximum in one expression | run-1286db1e2359 | 0.0199 | 21.2 | 84.3 |
| 30 | The names' output order sorted by one thread while the others start the hour pass, which needs it only at its lines | run-5d31ecdadb69 | 0.0198 | 20.9 | 84.4 |
| 31 | The hours sorted by size with each hour's size computed once (`sort_by_cached_key`) | run-c441864672ce | 0.0195 | 21.1 | 84.5 |
| 32 | Hours summarized smallest first, the smallest few last, into a latency buffer of untouched zero pages | run-13fb10f4c6ee | 0.0196 | 19.1 | 84.9 |
| 33 | One endpoint table that all threads share while they parse, instead of one per thread merged after it | run-9e334b5f00fa | 0.0190 | 18.9 | 85.3 |
| 34 | The padded copies allocated uninitialized, not zeroed, and each list of large latencies reserved once | run-2c0e91287411 | 0.0190 | 18.3 | 85.4 |
| 35 | The name hash reads two 8-byte windows of the name instead of three | run-b8eed3b9c923 | 0.0187 | 18.3 | 85.6 |
| 36 | Each record stored, and each padded copy read, without a bounds check | run-921f52908023 | 0.0183 | 18.5 | 85.7 |
| 37 | 5 fewer lines with the same machine code: the hour sizes summed inside `sort_by_cached_key`, the name hash's two cases as a `match`, and `same_name` folded into its one caller | run-9fe0786e93d3 | 0.0195 | 18.3 | 85.1 |
| 38 | A large latency's record field set in the branch that lists it, instead of by a `min` on every line | run-e44188ffe025 | 0.0197 | 18.5 | 84.9 |
| 39 | 5 fewer lines with the same machine code: the running sum of the hour's counts as one `for_each`, and a large latency listed and replaced by `LARGE` in one `bool::then` | run-89adcd077219 | 0.0196 | 18.4 | 85.1 |
| 40 | A plain static executable instead of a static PIE (`relocation-model=static`) | run-ebf9f6a49ad5 | 0.0190 | 17.9 | 85.5 |
| 41 | 4 fewer lines with the same machine code: the record store on one line, the counting pass without a binding for the endpoint, and each output line's length summed in one statement | run-32182846e438 | 0.0256 | 18.0 | 82.3 |
| 42 | 1 fewer line with the same machine code: the name hash binds the length in the arms of its `match` | run-a6b1091911cb | 0.0192 | 18.7 | 85.4 |
| 43 | 4 fewer lines on cold paths: `Interner::new` instead of an `impl Default`, the format queue as a lazy iterator, and a new name's padded copy from `array::from_fn` | run-a6b1091911cb | 0.0192 | 18.7 | 85.4 |
| 44 | 4 fewer lines in `summarize`: a chunk's start found through a one-line `newline` closure, and the format loop as one `for_each` over the queue | run-a6b1091911cb | 0.0192 | 18.7 | 85.4 |
| 45 | 2 fewer lines with the same machine code: the build stage mounts its sources instead of copying them, and `insert` stays out of line by `#[cold]` alone | run-379b43c0200f | 0.0189 | 18.7 | 85.5 |
| 46 | The input prefetched five lines ahead, `parse` out of line with a pointer for its loop, and the endpoint interner a static | run-7eca9cefcfac | 0.0186 | 18.2 | 85.8 |
| 47 | 5.8 instructions fewer per line: the probe counts the compare mask's trailing ones, the latency's digits combine in three steps, and their mask is one shifted constant | run-7eca9cefcfac | 0.0186 | 18.2 | 85.8 |
| 48 | An endpoint table of 16,384 slots instead of 8,192: 1.9% of lines take a second probe instead of 3.9% | run-7eca9cefcfac | 0.0186 | 18.2 | 85.8 |
| 49 | Three rare paths of the parse marked `std::hint::cold_path`, so its loop keeps its count, its end and its constants in registers | run-7eca9cefcfac | 0.0186 | 18.2 | 85.8 |
| 50 | Sorting networks unrolled level by level, and a network of 128 values instead of selection for groups of 65 to 128 values | run-9a8d6161d86a | 0.0235 | 18.0 | 83.2 |
| 51 | 6 fewer lines at the same speed: the parse's intrinsics from the file's one wildcard import, and the interner's padded copies zeroed in its static | run-b35557b63892 | 0.0180 | 18.8 | 85.9 |
| 52 | Endpoint names hashed with the CPU's CRC32C instruction instead of a multiply: 9.2 M instructions fewer | run-3367489cfd2c | 0.0176 | 18.6 | 86.3 |
| 53 | 9 fewer lines in the hour pass at the same speed: the large latency taken in `unwrap_or_else`, empty groups filtered out, the three selections in one `map`, the chunk's two releases in one | run-3367489cfd2c | 0.0176 | 18.6 | 86.3 |
| 54 | The latency's space bounded with `assert_unchecked`, so reading the status and ending the name take no bounds checks: 9.8 M instructions fewer | | | | |

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

## Step 26: 3 more lines

`digits` moves into `summarize_hour`, its only caller, as a one-line closure, and `percentiles`
reads `group.len()` where it used `n`. `.rodata` is byte-identical to step 25's; `.text` differs
in four compares of the group length, `cmp $9; jae` becoming `cmp $8; ja` and so on, the same test
in an instruction of the same size, so T and M cannot move. S 382 → 379. Gate: 307 pass, 0 fail.

## Step 27: the network's in-register stages without the copy

Each stage of `sort_network` compares every lane with a partner, and a stage reads the partners
from `old`, a copy of the registers taken before the stage, because a stage that pairs two
registers overwrites the first before the second reads it. A stage within one register (m < 8)
pairs lanes of the same register, which has not been written yet when its partners are read, so
it now permutes the register itself, and the compiler no longer stores and reloads it.

Instructions 774.28 → 770.72 M (−0.46%). Cycles of the network per call, rdtsc on an instrumented
copy, two runs: up to 16 latencies 125 → 117, up to 32 623 → 563, up to 64 1,414 → 1,230. The
percentiles and lines of an hour, timed per hour, 5 runs each: median 2.34 → 2.29 ms per thread.
That is about 0.06–0.1 ms per thread, under 0.5% of the run: user cycles 417.21 → 416.96 M over
15 interleaved runs and `ab.py` (61 runs per round, two rounds: medians 22.37 → 22.36 and
22.43 → 22.55 ms) are within noise. S unchanged at 379. Gate: 307 pass, 0 fail.

## Step 28: 3 more lines

The plan loop destructures each pair of runs from `array_windows()` instead of indexing a
`windows(2)` slice, and the two lints the code breaks on purpose, `inline_always` for `intern`
and `wildcard_imports` for the AVX2 intrinsics, are allowed in `Cargo.toml` with their reasons
instead of `#[expect]` on the items. `.text` and `.rodata` are byte-identical to step 27's, so T
and M cannot move. S 379 → 376. Gate: 307 pass, 0 fail.

Steps 27 and 28 went to the board in one push, so both rows cite the same run.

## Step 29: 1 more line

With the partner named `p` (step 27), the minimum and maximum fit inside the `blendv` call, so
the `(low, high)` binding goes. `.text` and `.rodata` are byte-identical to step 28's. S 376 → 375.
Gate: 307 pass, 0 fail.

The first board run of this code, run-9d734179de5e, drew 21.8 ms with the same machine code as
step 28; the row cites the next run of the same commit's code.

## Step 30: the name sort off the critical path

Timed with rdtsc inside the plan, the barrier leader took about 0.28 ms while 11 threads waited,
0.16–0.21 ms of it sorting the names into output order. No hour needs that order before its
lines, and a thread's first hour spends about 0.4 ms counting and scattering before them. So the
leader now publishes the runs and hour sizes first (`plan.set`), then sorts and publishes the
order in a second `OnceLock`, and `summarize_hour` waits on it only when it reaches its lines.

`ab.py` against step 29, interleaved: 101 runs twice, medians 22.58 → 22.37 and 22.46 → 22.26 ms,
minimums 21.59 → 21.49 and 21.93 → 21.63 ms; 60 runs on seed 1, 22.45 → 22.33 ms, and on seed 2,
22.56 → 22.23 ms. memory.peak medians 20.12 → 20.09 MB (5 runs each). S 375 → 377. Gate: 307
pass, 0 fail.

Not kept: giving out the last 10% of each thread's share as 48 pieces from a shared counter, so
threads that start late do less. It evened the parse's end spread from 1.08 to 0.36 ms but cost
1.5% (median 22.44 → 22.77 ms, user cycles +1.5%): a thread that finishes early already speeds up
its hyperthread sibling, and the extra merges queue on the interner's lock.

## Step 31: each hour's size computed once

The leader orders the hours by size for the hour pass, and `sort_by_key` summed an hour's run
lengths in every comparison, about 800 times for 72 hours. `sort_by_cached_key` sums each once.
The leader's plan before `plan.set`, timed with rdtsc over 31 interleaved runs: median 0.129 →
0.099 ms, minimum 0.105 → 0.083 ms. That is 0.13% of the run, below what `ab.py` resolves
(two rounds of 101: medians 22.34 → 22.37 and 22.22 → 22.30 ms). S unchanged at 377. Gate: 307
pass, 0 fail.

## Step 32: a lower peak from the order of the hours

Every thread of an instrumented copy read memory.current at each step of its work (local, turbo
off, seed 1, three runs). The peak was not at the end, where the output fills up, but at the
start of the hour pass. Each worker's first hour was one of the twelve largest, about 70,000
lines, and `resize` wrote zeros over its 280 KB latency buffer before the first latency, while
an hour's records go back to the kernel only after their scatter. Within 0.6 ms of the start,
memory.current rose by 2.6–3.1 MiB to 19.4–20.5 MiB. The parse had topped out at 17.8–18.4 MiB
and the finished output at 19.3–19.5 MiB.

Two changes:

- Each worker's latency buffer is allocated once, with room for every line of the log
  (`vec![0; len / MIN_LINE + 1]`, 24 MB of address space). glibc's malloc maps a request that
  large as fresh zero pages, which `calloc` leaves untouched, so a page takes memory only once a
  latency lands in it.
- The hours go smallest first, so each buffer grows a little at a time while earlier hours free
  their records. The `threads` smallest hours go last instead, so every worker ends on a short
  hour, as with largest first.

In four runs of the same probe, the hour pass now tops out 0.2–1.2 MiB above where it starts, at
17.5–18.0 MiB, and the output at 16.8–17.2 MiB. The peak is the end of the parse, 17.6–18.3 MiB,
where the threads still parsing hold their name tables (about 190 KB each) on top of the
records. Local, turbo off: memory.peak, 20 alternating runs each, median 19.89 → 18.15 MiB on
seed 1; 10 each on seed 2, 19.97 → 17.96 MiB. `ab.py`, 61 interleaved runs, two rounds on seed
1: medians 22.56 → 22.33 and 22.72 → 22.53 ms, minimums 21.49 → 21.53 and 21.80 → 21.89 ms; one
round on seed 2: median 21.49 → 21.25 ms, minimum 20.98 → 20.89 ms. S 377 → 378. Gate: 310 pass,
0 fail (full seeds 1–7 and 10–12, 300 tiny seeds).

Tried and dropped, medians of memory.peak over 8 to 12 alternating runs, local, turbo off:

- The untouched buffer alone, largest first: 19.84–19.90 MiB. Most endpoints have a few
  latencies an hour, and their places are spread over the whole buffer, so its pages fill early
  anyway.
- Smallest first all the way: 17.91–18.04 MiB, but the longest hours then start last and the
  threads finish apart: `ab.py` medians +0.9% and +1.9% (60 runs, two rounds).
- The 12 or the 24 smallest hours first, then largest first: 19.56 and 19.03 MiB.
- The kept order with the buffer grown by `resize`: 18.72 MiB.
- `malloc_trim(0)` after each thread's merge, to give its freed name table back: 17.72 against
  18.04 MiB in one session, 18.01 against 17.99 MiB in another.

## Step 33: one endpoint table for all threads

Each thread used to give endpoint names ids in a table of its own, about 190 KB once written
(slots, names and padded copies), then merge it into a shared table under a lock, and the hour
pass mapped every record's chunk id to a shared id through one more table per chunk. Now all
threads give out ids in one table while they parse (`Interner`). A lookup takes no lock: it
loads a slot with `Acquire`, and a thread that adds a name writes the name and its padded copy
before it stores the id with `Release`. Adding takes a mutex. Under it, the thread checks that
the empty slot it found is still empty; if another thread filled it in the meantime, it probes
again. The records hold shared ids, so the merge and the id table go.

The same probe as in step 32 (four runs, local, turbo off, seed 1): the end of the parse falls
from 17.6–18.3 to 16.2–16.4 MiB. The hour pass is now the peak, at 17.9–18.2 MiB: the per-thread
tables had left freed memory in the threads' malloc arenas, which the hour pass's small
allocations reused, and now those come from new pages. The output ends at 16.6–17.0 MiB.

Local, turbo off: memory.peak, 20 alternating runs, median 18.14 → 17.65 MiB on seed 1; 10 each
on seed 2, 17.91 → 17.81 MiB. `ab.py`, 61 interleaved runs, two rounds on seed 1: medians
22.28 → 21.54 and 22.20 → 21.42 ms, minimums 21.44 → 20.94 and 21.35 → 20.83 ms; one round on
seed 2: median 21.77 → 21.13 ms, minimum 21.30 → 20.49 ms. `perf stat` (10 runs, twice):
773.7 → 747.8 million user instructions, task clock 220.1 → 212.7 ms. The hour pass no longer
loads and bounds-checks an id for each record, twice, and the merge and the per-thread inserts
are gone. S 378 → 381. Gate: 310 pass, 0 fail (full seeds 1–7 and 10–12, 300 tiny seeds).
Seeds 1–3, 15 runs each on 12, 4 and 2 CPUs: all 135 outputs match the reference hash.

## Step 34: two buffers the allocator had filled

A copy of `/proc/self/smaps` at the barrier after the parse (local, seed 1, two runs) counted
13,312–13,324 KiB of anonymous memory, 12,144 KiB of it records. Two parts of the rest needed
no memory:

- The padded copies. `Box::new_zeroed_slice` gets a block aligned to 64 bytes from
  `posix_memalign` and then writes zeros over it, so all 256 KiB of the table were in memory,
  though the 1,842 copies fill 115 KiB. The table now holds `MaybeUninit` entries and comes from
  `Box::new_uninit_slice`, whose pages take memory only as copies are written: 260 → 116 KiB.
- The lists of large latencies. Each grew by doubling inside its thread's malloc arena, and
  every buffer it outgrew stayed there. Reserved like the records, with room for a large
  latency on every line, a list is a mapping of its own whose untouched pages cost nothing. The
  thread arenas went from 424–436 to 96 KiB, the main heap from 308–312 to 148 KiB.

At the barrier: 13,312–13,324 → 13,020 KiB. Local, turbo off: memory.peak, 20 alternating runs,
median 17.66 → 17.36 MiB on seed 1; 10 each on seed 2, 17.58 → 17.38 MiB. `ab.py`, 61
interleaved runs, two rounds on seed 1: medians 21.46 → 21.43 and 21.48 → 21.52 ms, minimums
21.01 → 21.02 and 21.02 → 21.05 ms. S 381 → 382. Gate: 310 pass, 0 fail (full seeds 1–7 and
10–12, 300 tiny seeds).

The peak is still the start of the hour pass. In the same probe as step 33, anonymous memory
rises by 1.1 MiB within 0.2 ms of it, as every worker writes its first latency buffer while
the records are all still there, and memory.current tops out 1.4–1.8 MiB above the end of the
parse. Tried and dropped, medians of memory.peak over 20 alternating runs, local, turbo off:

- Hours smallest first all the way, without moving the smallest to the end: 0.26–0.32 MiB
  lower, but `ab.py` median 21.50 → 21.72 ms (+1.0%, 61 runs).
- The 12 smallest hours first and the next 12 last: 0.14 MiB higher than the kept order.
- Each hour's lines released once formatted (1 line): in the probe the end of the output falls
  from 16.6–17.0 MiB (step 33) to 14.3–14.7 MiB, but the hour pass still sets the peak (17.47
  against 17.42 MiB without it).

## Step 35: two windows per name hash

A phase split of step 34 (instrumented build, 30 runs, local, turbo off; ms from before the
fork): `main` starts at 0.57, the first worker at 0.80 and the last at 1.33. The parse takes
11.1–11.7 ms per thread and ends between 12.39 and 12.64, the plan 0.07. The hour pass ends at
17.12–17.55: per thread, counting 0.60 ms, the scatter 1.10, releasing records 0.24, and
percentiles and lines 2.41. Formatting takes 1.67 ms per thread, releasing the input 0.59, the
last thread calls `exit` at 20.40, and the parent's `wait` returns 0.79 ms later.

`perf stat` counts 238.5 user instructions per line at 1.99 per cycle on each hyperthread, with
0.05 L2 misses per line and 1.4% of cycles stalled on an L1 miss: the parse is bound by
instructions, not by memory. The name hash read three 8-byte windows of the name, and the middle
one cost a subtraction, a shift, a bounds check, a load, a rotate and an XOR on every line. Now
it reads the first and the last only. In a simulation of the 8,192-slot table, 3.6–4.7% of lines
need a second probe instead of 3.1–3.4% (seeds 1, 3 and 5), each a mispredicted branch: far less
than the instructions saved.

Local, turbo off: instructions 739.4 → 714.2 M, branch misses 1.09 → 1.08 M. `ab.py`, 61
interleaved runs, four rounds: medians 21.58 → 21.12, 21.53 → 21.14, 21.58 → 21.11 and
21.56 → 21.22 ms, minimums 21.10 → 20.62, 20.93 → 20.55, 20.98 → 20.39 and 21.17 → 20.74 ms.
memory.peak, 12 alternating runs: median 18.17 → 18.14 MB. S unchanged at 382. Gate: 310 pass,
0 fail.

## Step 36: two bounds checks fewer per line

Every line paid for two bounds checks that could never fail, whatever the input. The store of
its record: `line_end` starts looking at byte `MIN_LINE - 1`, so each line, malformed or not,
moves on by at least `MIN_LINE` bytes, and the chunk's `len / MIN_LINE + 1` records always have
room. And the padded copy read for every probe: an id in a slot is below the count of names,
which `insert` keeps within the 4,096 copies. Both are now unchecked, with that argument as the
SAFETY note. The other checks on the line stay: the name and status offsets come from the
line's bytes, and only well-formed input keeps them in range.

The compiler's register allocation decided the gain. Shifting the error bit where it is
computed, rather than in the record's expression, gave the same instruction count, but the loop
then reloaded a pointer from the global offset table on every line instead of keeping it in a
register, and gained about 0.05 ms instead of 0.25.

Local, turbo off: instructions 714.2 → 701.5 M (4 per line), cycles 375.6 → 371.7 M (5 runs).
`ab.py`, 61 interleaved runs, four rounds: medians 21.16 → 20.89, 21.14 → 20.91, 21.17 → 20.90
and 21.13 → 20.93 ms, minimums 20.67 → 20.30, 20.60 → 20.18, 20.61 → 20.51 and 20.80 → 20.50 ms.
memory.peak, 11 alternating rounds on each of seeds 1–7 and 10–12: median 18.09 → 18.07 MB.
S 382 → 385. Gate: 310 pass, 0 fail.

## Step 37: 5 fewer lines

The plan hands the closure that sums an hour's run lengths straight to `sort_by_cached_key`
instead of binding it first. `hash` picks between the masked short name and the last window
with a `match` on the length, whose two arms rustfmt keeps on two lines where the `if`/`else`
took four. `same_name`, a one-line function with one caller, is folded into `Interner::intern`,
whose doc now carries its explanation. `.text` and `.rodata` are byte-identical to step 36's, so
T and M cannot move. S 385 → 380. Gate: 310 pass, 0 fail. The board's run of this code, run-9fe0786e93d3,
drew 19.5 ms, against 18.3 ms for step 36's identical machine code: a slow draw.

## Step 38: no `min` on every line

The record's latency field was `micros.min(LARGE)`, a compare and a conditional move on
every line, though the branch just before it, which lists a large latency, has already decided
the same thing. That branch now sets the field to `LARGE` itself, and other lines store the
latency as it is.

Local, turbo off: instructions 701.5 → 692.3 M (3 per line). `ab.py`, 61 interleaved runs, six
rounds: medians 20.98 → 20.91, 21.04 → 20.91, 20.98 → 20.85, 20.99 → 20.77, 20.97 → 20.85 and
20.98 → 20.88 ms, minimums 20.37 → 20.28, 20.24 → 20.02, 20.50 → 20.39, 20.46 → 20.03,
20.22 → 20.16 and 20.56 → 20.28 ms. memory.peak, 11 alternating rounds on each of seeds 1–7 and
10–12: median 18.08 → 18.13 MB, with no change to any allocation. S 380 → 381. Gate: 310 pass,
0 fail.
The board measured this code in the run of commit 3e78ba2, which changed only track C:
run-e44188ffe025 drew 19.7 ms, against 18.3 ms for step 36's code, a slow draw.

## Step 39: 5 fewer lines

The running sum that turns `summarize_hour`'s counts into ends is one `for_each` over the range
instead of a three-line `for` loop. `parse` lists a large latency and leaves `LARGE` in its place
with one `bool::then`, whose `mem::replace` hands the old value to the side list, instead of a
four-line `if`. `.text` and `.rodata` are byte-identical to step 38's, so T and M cannot move.
S 381 → 376. Gate: 310 pass, 0 fail.
The board's run, run-89adcd077219, drew 19.6 ms: the fourth draw of 19.5–19.7 ms in a row since
step 36's 18.3 ms, on code that has changed by 3 instructions per line since.
A second run of the same code, run-a0af33fbe24c (commit f25c5c2, which changed only track C),
measured 19.7 ms, 18.1 MB and 85.1 points.

## Step 40: a plain static executable

The binary was a static PIE: the kernel loads it at a random address and it relocates itself
before `main`. Built with `-C relocation-model=static`, it is a plain static executable at a
fixed address instead, with the same code apart from addressing: 692.3 → 689.4 M instructions.
Yet it ran 3.9% fewer user cycles (383.7 → 368.8 and 382.7 → 368.5 M, 10 runs each), with
about as many dTLB load misses (81,200 → 77,700) and half the iTLB misses (15,300 → 7,500).
Kernel cycles and page faults did not move. The phase split puts all of it in the parse: 10.64
→ 10.03 and 10.55 → 10.06 ms per thread (two sets of 30 runs), while start-up, the hour pass
and formatting kept their lengths. It is not the alignment of the code: aligning every function
of the PIE build to 64 bytes (`-align-all-functions=6`) changed nothing.

Local, turbo off: `ab.py`, 61 interleaved runs against step 38's code, four rounds: medians
20.47 → 19.97, 20.48 → 19.93, 20.62 → 20.13 and 20.90 → 20.36 ms, minimums 20.17 → 19.61,
19.96 → 19.65, 20.15 → 19.50 and 20.27 → 19.59 ms; against step 39's build, two clean rounds:
20.59 → 20.09 and 20.80 → 20.48 ms. memory.peak, 11 alternating rounds on each of seeds 1–7 and
10–12: median 18.18 → 18.21 MB. S unchanged at 376. Gate: 310 pass, 0 fail.

## Step 41: 4 fewer lines

With no `min` in it since step 38, `parse`'s record store fits on one line as an `unsafe` block
around the assignment. `summarize_hour`'s counting pass indexes both counters with the record's
endpoint directly instead of binding it first. Its `width` closure counts the space before a
number with the number's digits, so an output line's length is one statement, which sums the
numbers first: summed after the name's length, the same arithmetic compiled to different code.
`.text` is byte-identical to step 40's, and `.rodata` differs only in the line numbers of 30
panic locations, which the plain static executable of step 40 keeps there, so T and M cannot
move. S 376 → 372. Gate: 310 pass, 0 fail.
The board's run, run-32182846e438, drew 25.6 ms against 19.0 ms for step 40's identical `.text`.
In the same push, track D's unchanged code drew 132.6 ms against its usual 122–128 ms: the box
was slow for that run.

## Step 42: 1 fewer line

`hash` binds the name's length in the arms of its `match` instead of in a `let` before it.
`.text` is byte-identical to step 41's, and `.rodata` differs only in the line numbers of panic
locations, so T and M cannot move. S 372 → 371. Gate: 310 pass, 0 fail.

## Step 43: 4 fewer lines on cold paths

- The interner's constructor is `Interner::new`, first in its `impl`, instead of an
  `impl Default` of its own, which only `summarize` called.
- The format queue is the iterator itself, in its mutex: each `next` cuts the hour's place from
  the rest of the output as it hands the hour out, so no vector of places is collected first.
  The hours now go out first to last; `pop` handed them out last to first.
- `insert` builds a new name's padded copy with `array::from_fn`, which keeps byte `i` of the
  window for `i <= n`, instead of copying the window and zeroing its tail with `fill`.

The machine code changes in `Interner::insert`, which runs 1,842 times a seed, and in the work
closure from the format queue on; elsewhere only addresses of constants and stack slots moved.
`ab.py` against step 41, two rounds of 60 runs: medians 20.15 → 20.10 and 19.99 → 19.85 ms,
minimums 19.50 → 19.34 and 19.44 → 19.24 ms. memory.peak, 24 alternating runs on seed 1: median
18.29 → 18.13 MB. S 371 → 367. Gate: 310 pass, 0 fail.

## Step 44: 4 fewer lines in `summarize`

`start` finds a chunk's first line through a one-line `newline` closure, the position of the LF
before the chunk's nominal start, instead of a block with a binding of its own. The threads take
hours from the queue in one `iter::from_fn(...).for_each(...)` instead of a `while let` over a
named closure; the lock is still taken and released inside the closure, so no thread holds it
while it formats. The machine code changes only around those lines of the work closure and in
its stack slots before the hour pass. `ab.py`, 40 interleaved runs, two rounds: medians
20.20 → 20.07 and 20.16 → 20.12 ms against step 43 (step 41's code: 20.25 and 20.25 ms in the
same rounds). memory.peak, 24 alternating runs on seed 1: median 18.13 MB for both; on seeds 3, 7
and 11, 8 runs each, within 0.02 MB of step 41. S 367 → 363. Gate: 310 pass, 0 fail.
The board measured steps 42 to 44 together, pushed as one, in run-a6b1091911cb: 19.2 ms.

## Step 45: 2 fewer lines

The Dockerfile's build stage mounts its context (`RUN --mount=type=bind,target=/src`) instead of
copying it with `COPY . .`, and cargo builds into `--target-dir /out`, since the mount is
read-only. `harness/dockerfile_check.py` accepts a bind mount of the context, and every builder
that builds us is BuildKit: Depot, the board's rootless BuildKit fallback and final scoring's
`docker build`. A local `docker build --network=none` of the old and the new Dockerfile gave
byte-identical binaries. `insert` drops `#[inline(never)]`: `#[cold]` alone keeps it out of
line. `.text` is byte-identical to step 44's, and `.rodata` differs only in the line numbers of
27 panic locations, so T and M cannot move. S 363 → 361. Gate: 310 pass, 0 fail.

## Step 46: the input prefetched, and `parse` on its own

`parse` was inlined into the work closure, and its speed depended on where its loop landed. A
change elsewhere in that function, or one that only moved the loop, raised the L1 misses by half,
all on the input: `L1-dcache-load-misses:u` on seed 1 was 4.27 M for step 45, and 6.72–6.74 M both
with `parse` out of line and with a change to `hour_index` alone, whose cycles:u rose 3.7–4.1%
though they ran about 6.4 M fewer instructions (battery, `perf stat`, 40 interleaved runs, two
rounds). Step 45's layout was the lucky one: the hardware prefetcher kept the input in L1 for it
and not for the others. Five changes:

- Each line prefetches the two cache lines from 384 bytes on (`_mm_prefetch` with `_MM_HINT_T0`),
  about five lines ahead. With them the misses are 3.85 M in every layout tried: 3.85–3.86 M in a
  copy whose `parse` an `#[inline(never)]` on `summarize_hour` moved 0x1b0 bytes on, and its
  cycles:u matched too (346.6 and 346.1 M against 346.6 and 347.1 M).
- `parse` is `#[inline(never)]`. The work closure realigns its stack to 32 bytes and so keeps `rbp`
  as its frame pointer; out of line, the loop has `rbp` for itself.
- The loop moves a pointer to the line instead of an index into the log: 9.7 M instructions fewer.
- The status bit and a large latency are taken before the lookup, so fewer values live through it.
- The interner is a static, `ENDPOINTS`, so its tables lie at fixed addresses: a probe loads no
  pointer to them, and `parse` keeps no register for them: 9.6 M instructions fewer. A `OnceLock`
  starts out nonzero, so the tables take 393 KB of `.data`; only the pages that are touched are
  read in, as with the heap pages they took before. The static alone gained nothing on step 45's
  code (see the experiments below): inlined into the work closure, the loop had no register to give
  back.

`parse` now has its own symbol, 0x8f1 bytes; the work closure shrank from 0x40ba to 0x36de bytes.

On AC power (local, P-cores, turbo off), `ab.py`, 61 interleaved runs, four rounds, against step
45: medians 20.33 → 19.73, 20.23 → 19.70, 20.21 → 19.70 and 20.25 → 19.74 ms (−2.5 to −3.0%),
minimums 19.67 → 19.12, 19.43 → 19.07, 19.63 → 19.18 and 19.50 → 19.04 ms. Timed by fork, exec and
wait, as the board's timer does, 41 runs, two rounds: medians 19.07 → 18.50 and 19.09 → 18.47 ms.
Without the static, the other four changes measured 19.81, 19.83, 19.83 and 19.78 ms in the same
rounds, and 18.66 and 18.66 ms by the second timer. Counters, measured on battery power, so only
the counts compare (`perf stat`, 40 interleaved runs, two rounds): instructions:u 690.9 → 662.6 M
(−4.1%), cycles:u medians 362.8 → 346.6 M and 363.0 → 347.1 M (−4.4%), minimums 351.2 → 326.9 M and
347.7 → 328.3 M, L1-dcache-load-misses:u 4.27 → 3.85 M. memory.peak, five alternating rounds on
seeds 1, 2, 3, 5, 7, 10 and 11: median 18.09 → 18.09 MB. S 361 → 367. Gate: 310 pass, 0 fail; full
seeds 1–7 and 10–12 give the reference hashes; clippy reports no warnings.

## Step 47: 5.8 instructions fewer per line

Three cuts in the probe and the latency, each measured alone (instructions:u, seed 1):

- **The probe counts the compare mask's ones.** A probe matches when the first `n + 1` bytes agree,
  the name and the space: when the mask starts with more than `n` ones. `trailing_ones() > n` ran
  5.5 M instructions fewer, 1.8 per line, than masking off the bits above `n` and testing the rest.
- **The digits in three steps.** `eight_digits` combines the digit lanes into pairs, the pairs into
  fours and the fours into the number, each with one multiply by `k·2^s + 1`, a shift and a mask,
  for k = 10, 100 and 10,000. Step 45's form, which took the pairs into the number with two
  multiplies side by side, ran 9.2 M instructions more, 3 per line.
- **The digit mask as one shifted constant**, `0x0F0F_0F0F_0F0F_0F0F << s`, instead of the constant
  and a shifted all-ones mask: 2.9 M instructions fewer.

On AC power (local, P-cores, turbo off), `ab.py`, 61 interleaved runs, four rounds, against step
46: medians 19.73 → 19.54, 19.70 → 19.61, 19.70 → 19.63 and 19.74 → 19.54 ms (−0.4 to −1.0%),
minimums 19.12 → 18.92, 19.07 → 19.06, 19.18 → 19.08 and 19.04 → 18.91 ms; timed as the board does,
41 runs, two rounds, medians 18.50 → 18.46 and 18.47 → 18.47 ms. Counters, measured on battery
power, so only the counts compare (`perf stat`, 40 interleaved runs, two rounds): instructions:u
662.6 → 644.6 M (−2.7%), cycles:u medians 346.6 → 343.4 M and 347.1 → 342.9 M (−0.9 to −1.2%),
minimums 326.9 → 330.6 M and 328.3 → 313.7 M, L1-dcache-load-misses:u 3.85 → 3.85 M. A copy whose
`parse` an `#[inline(never)]` on `summarize_hour` moved 0x1b0 bytes on ran the same: cycles:u 344.5
and 342.2 M against 345.8 and 343.1 M in another session, L1 misses 3.85 M. memory.peak, as in step
46: median 18.09 → 18.00 MB. S 367 → 366. Gate: 310 pass, 0 fail; full seeds 1–7 and 10–12 give the
reference hashes; clippy reports no warnings.

## Step 48: an endpoint table of 16,384 slots

With 8,192 slots, 3.9% of seed 1's lines took a second probe, and each one is a mispredicted
branch. A simulation of the table, names inserted in file order, gives 1.9% with 16,384 slots:
4.20% → 2.27% on average over seeds 1–7 and 10–12, and at most 3.33% (seed 11). branch-misses:u
went 1.09 → 1.01 M a run (`perf stat`, 10 interleaved runs). The slots take 32 KB more of `.data`,
64 KB in all, and the L1 misses rose 3.85 → 3.93 M.

`ab.py` cannot tell this step from step 47. On AC power (local, P-cores, turbo off), 61 interleaved
runs, four rounds: medians 19.54 → 19.54, 19.61 → 19.60, 19.63 → 19.65 and 19.54 → 19.49 ms,
minimums 18.92 → 19.03, 19.06 → 18.90, 19.08 → 18.93 and 18.91 → 18.67 ms; timed as the board does,
41 runs, two rounds, medians 18.46 → 18.49 and 18.47 → 18.36 ms. Without the static interner of
step 46, the same change measured 19.72 → 19.53, 19.76 → 19.62, 19.75 → 19.52 and 19.65 → 19.58 ms
in the same rounds. Counters, measured on battery power, so only the counts compare (`perf stat`,
40 interleaved runs, two rounds): instructions:u 644.6 → 643.2 M (−0.2%), cycles:u medians
343.4 → 340.9 M and 342.9 → 341.7 M (−0.4 to −0.7%), and 345.8 → 342.1 M and 343.1 → 340.7 M in a
second session, minimums 330.6 → 327.2 M and 313.7 → 321.9 M. A copy moved as in step 47 ran the
same: cycles:u 342.1 and 340.3 M against 342.1 and 340.7 M, L1 misses 3.94 M. memory.peak: median
18.00 → 18.12 MB, and 18.40 → 18.26 MB in an earlier session. S 366 → 366. Gate: 310 pass, 0 fail;
full seeds 1–7 and 10–12 give the reference hashes; clippy reports no warnings.

## Step 49: three rare paths marked cold

`std::hint::cold_path()` marks a new run, a latency of 8 digits or more and a second probe. Without
the hints, LLVM's guess at how often each runs kept the loop's record count and its end on the
stack and rebuilt the LF pattern and two 64-bit constants on every line; with them, those stay in
registers, and the hot path has two taken branches fewer per line (br_inst_retired.near_taken:u
40.9 → 34.7 M): 26.0 M instructions fewer, 8.4 per line. Each hint alone, on an earlier form of
step 48: the second probe −16.7 M, the long latency −3.4 M, the new run −0.1 M; the three together
−26.0 M. Hints on a large latency, on a line of more than 103 bytes and on a name of 64 bytes or
more gained nothing.

On AC power (local, P-cores, turbo off), `ab.py`, 61 interleaved runs, four rounds, against step
48: medians 19.54 → 19.36, 19.60 → 19.41, 19.65 → 19.37 and 19.49 → 19.33 ms (−0.8 to −1.4%),
minimums 19.03 → 18.49, 18.90 → 18.82, 18.93 → 18.62 and 18.67 → 18.81 ms; timed as the board does,
41 runs, two rounds, medians 18.49 → 18.25 and 18.36 → 18.14 ms. A copy whose `parse` an
`#[inline(never)]` on `summarize_hour` moved 0x170 bytes on timed the same: medians 19.35, 19.40,
19.40 and 19.36 ms, and 18.16 and 18.31 ms. Counters, measured on battery power, so only the counts
compare (`perf stat`, 40 interleaved runs, two rounds): instructions:u 643.2 → 617.2 M (−4.0%),
cycles:u medians 340.9 → 334.1 M and 341.7 → 333.3 M (−2.0 to −2.5%), minimums 327.2 → 321.1 M and
321.9 → 318.0 M, L1-dcache-load-misses:u 3.93 → 3.94 M; the moved copy 333.8 and 333.6 M.
memory.peak: median 18.12 → 18.17 MB. S 366 → 369. Gate: 310 pass, 0 fail; full seeds 1–7 and 10–12
give the reference hashes; clippy reports no warnings.

Steps 46 to 49 together, in the same rounds: medians 20.33 → 19.36, 20.23 → 19.41, 20.21 → 19.37
and 20.25 → 19.33 ms (−4.1 to −4.8%), and 19.07 → 18.25 and 19.09 → 18.14 ms timed as the board
does. Without the static interner the four steps end level with these, at 19.40, 19.40, 19.44 and
19.27 ms and at 18.17 and 18.19 ms, with 8.4 M instructions more; the static measured ahead at
steps 46 and 47, so it stayed.

Steps 46 to 49 reached the board in one push. Its run, run-7eca9cefcfac, measured 18.6 ms and
18.2 MB, against 18.8 and 19.1 ms for step 45's code in its last two runs (run-6e4daa8e2dbc,
run-8a8f22b6a3dc).

## Step 50: sorting networks unrolled level by level

The network's two loops become one call per merge level, `merge::<R, L>`, each a loop of constant
trip count `L` around a `stage` function. LLVM unrolls those loops before it inlines the stages, so
every stage sees a constant partner distance: its masks are constants, each register's partner is a
fixed register, and the networks of 32, 64 and 128 values come out without loops, blends on runtime
masks or copies to the stack. A network of 128 values (`R = 16`) then beats selection too, and takes
the groups of 65 to 128 values, 2.4% of them. In a microbenchmark over seed 1's 126,908 groups (one
thread, ns per group), 17–32 values went 99.5 → 52.7, 33–64 values 253 → 100, and 65–128 values 687
(selection) → 389–432; all groups 15.05 → 12.87 ms. As a loop, the 128-value network took 923 ns,
more than selection.

On step 45 the same change lost 2% (`ab.py`, 61 interleaved runs, medians 20.15 → 20.57 and
20.17 → 20.53 ms) although it ran 17 M instructions fewer: the parse was then inlined in the work
closure, and the larger closure gave it other registers and stack slots. With the parse out of line
in both builds, the networks took 2.5–2.9% fewer cycles:u. Since step 46 the parse is a function of
its own, and here its instructions are step 49's; only the address of the endpoint table in them
changes.

`ab.py`, 61 interleaved runs, four rounds, against step 49: medians 19.39 → 19.09, 19.34 → 19.08,
19.36 → 19.14 and 19.34 → 19.03 ms (−1.1 to −1.6%), minimums 18.71 → 18.32, 18.74 → 18.36,
18.93 → 18.52 and 18.76 → 18.43 ms; the first two rounds ran a build that also had `#[inline]` on
`merge` and `stage`, with a byte-identical `.text`. `perf stat`, 10 interleaved rounds of 3 runs:
instructions:u 616.0 → 598.9 M (−2.8%), cycles:u median 337.8 → 332.7 M (−1.5%). memory.peak, five
alternating rounds on seeds 1, 2, 3, 5, 7, 10 and 11: median 18.34 → 18.24 MB. S 369 → 379. Gate:
310 pass, 0 fail; full seeds 1–7 and 10–12 give the reference hashes; clippy reports no warnings.

The board's run, run-9a8d6161d86a, drew 23.5 ms, a slow draw: in the same run, track C's unchanged
code drew 60.1 ms, against its usual 27 ms, and track D's 135.9 ms, against 122 to 127 ms.
A second run of the same commit, run-65986567a603, drew 17.9 ms, and a third, run-148a82d5f70d,
17.6 ms.

## Step 51: 6 fewer lines

`parse` and `equal_mask` lose their own `use std::arch` lines: the wildcard import that step 50 put
at the top of the file brings in the same intrinsics, and `equal_mask` names them in full, with its
two halves in one `map`. The interner's padded copies are `UnsafeCell<Padded>` instead of
`UnsafeCell<MaybeUninit<Padded>>` and start out as zeros, written in the static's initializer, so
`Interner::new` goes, and with it the two `assume_init_ref` calls and the `unsafe` block that made
the uninitialized array. The static is in `.data` anyway, since a `OnceLock` starts out nonzero, and
`.data` keeps its size. `.text` and `.data` are byte-identical to step 50's, and `.rodata` differs
only in the line numbers of panic locations, so T and M cannot move: `ab.py`, seven interleaved
rounds (three of 41 runs, four of 61), medians 19.25 → 19.12, 19.16 → 19.11, 19.03 → 18.96,
19.06 → 19.03, 18.99 → 19.05, 19.06 → 19.03 and 19.69 → 19.62 ms. S 379 → 373. Gate: 310 pass,
0 fail.

The board drew 18.0 ms at 18.8 MB for it in run-b35557b63892, 19.5 ms for the same code in
run-dbba1dbc4ea4, and 17.7 ms at 18.2 MB in run-d68025a3c15c.

## Step 52: a CRC32C name hash

`hash` mixes the name's length with its first and last 8-byte windows through SSE4.2's CRC32C
instruction, `_mm_crc32_u64`, instead of rotating the last window, combining it with the first by
XOR and multiplying: the length is the CRC's starting state, and each window is one step, so a name
of under 8 bytes takes one step and a longer name two. The interner takes its slot from the CRC's
top 14 bits, 18 to 31. LLVM knows the CRC to be under 2^32, so the probe still needs no bounds
check. In a simulation of the 16,384-slot table, 1.9–2.5% of lines take a second probe instead of
1.9–2.3% (seeds 1, 3 and 5), and 2.2–3.0% with the CRC's low 14 bits. Seed 1's share grows most,
1.9 → 2.5%, and its branch-misses:u with it, by 0.02 M.

Local, AC power, turbo off. `perf stat`, 30 interleaved runs: instructions:u 600.9 → 591.7 M
(−1.5%), cycles:u median 328.8 → 324.1 M (−1.4%), branch-misses:u 0.898 → 0.916 M,
L1-dcache-load-misses:u 3.95 → 3.98 M. `ab.py`, 61 interleaved runs, four rounds: medians
19.01 → 18.84, 18.92 → 18.78, 19.06 → 18.85 and 19.24 → 19.20 ms; seven earlier rounds of builds
with the same `.text` as steps 51 and 52 (three of 41 runs, four of 61): 19.12 → 18.97,
19.11 → 18.86, 18.96 → 18.80, 19.03 → 18.96, 19.05 → 18.85, 19.03 → 18.87 and 19.62 → 19.04 ms.
memory.peak, three batches of alternating rounds on seeds 1, 2, 3, 5, 7, 10 and 11 (3, 8 and 10
rounds): medians 18.21 → 18.41, 18.27 → 18.37 and 18.26 → 18.16 MB, where two more builds with the
same `.text` as the two drew 18.24 and 18.21 MB: noise, as the hash allocates nothing. S 373 → 372.
Gate: 310 pass, 0 fail; full seeds 1–7 and 10–12 give the reference hashes; clippy reports no
warnings.
## Step 53: 9 fewer lines in the hour pass

Four rewrites in the hour pass and the work closure's tail:

- The scatter takes a record's latency bits unless they hold `LARGE`, and the next large latency
  in `unwrap_or_else` when they do, instead of a four-line `match`: 3 lines.
- `summarize_hour` takes only the endpoints with lines from the order, with a `filter`, instead of
  wrapping the loop's body in an `if`: 2 lines.
- The three selections of a large group are one `map` over (start, rank) pairs, each selecting in
  the suffix that starts at the rank before: 3 lines.
- The work closure frees its chunk's records and large latencies in one `map`: 1 line.

With the `filter` or the `map`, LLVM no longer inlines `percentiles` into the work closure. It
becomes a function of its own, 21.7 KB with the networks, and the work closure is inlined into
`fan_out` instead. Either rewrite alone, and both together, run the same 3.6 M instructions:u
more, so that is the cost of the new layout, and it costs no cycles: `perf stat`, 10 interleaved
rounds of 3 runs, instructions:u 598.8 → 602.4 M, cycles:u median 330.0 → 330.4 M.
`#[inline(always)]` on `percentiles`, one line back, moved `summarize_hour` out of line instead
and timed the same.

`ab.py`, 61 interleaved runs, eight rounds against step 51: medians 19.03 → 18.94,
18.91 → 18.84, 19.01 → 19.04, 19.02 → 18.98, 19.05 → 19.00, 19.04 → 19.03, 19.06 → 18.96 and
19.11 → 19.01 ms (−0.3% on average), minimums 18.53 → 18.32, 18.39 → 18.35, 18.27 → 18.45,
18.41 → 18.37, 18.49 → 18.40, 18.65 → 18.54, 18.65 → 18.53 and 18.53 → 18.31 ms. memory.peak,
five alternating rounds on seeds 1, 2, 3, 5, 7, 10 and 11, three times: medians 18.17 → 18.28,
18.05 → 18.09 and 18.29 → 18.23 MB, from the same allocations. S 373 → 364. Gate: 310 pass,
0 fail; full seeds 1–7 and 10–12 give the reference hashes; clippy reports no warnings.

## Step 54: the latency's space bounded

`latency` returns the index of the line's last space, and the parse reads the status at `space - 3`
and ends the name at `space - 4`. On the fast path the space lies 1 to 8 bytes below the LF, from
byte 31 on. The slow path, for 8 to 10 digits, now lifts it to at least 4, which only a malformed
line can need. `std::hint::assert_unchecked` hands the parse that range, `4..LINE`, and LLVM drops
the bounds checks of the status's byte and of the name's end: 3.1 instructions fewer per line.

Local, AC power, turbo off, on step 53's code. `perf stat`, 30 interleaved runs: instructions:u
595.2 → 585.4 M (−1.6%), cycles:u median 321.2 → 320.9 M, L1-dcache-load-misses:u 3.98 → 3.98 M.
`ab.py`, 61 interleaved runs, six rounds: medians 18.92 → 18.88, 18.80 → 18.77, 18.84 → 18.79,
18.89 → 18.79, 18.86 → 18.81 and 18.79 → 18.75 ms, −0.05 ms (−0.3%) on average. On step 52's code,
before step 53 went in, the same change ran instructions:u 591.5 → 581.8 M and cycles:u
324.3 → 322.3 M, and −0.11 ms (−0.6%) on average in eleven rounds of 61 runs (listed with the
pointer form under "Experiments that did not stay"). memory.peak, eight alternating rounds on seeds
1, 2, 3, 5, 7, 10 and 11: median 18.26 → 18.30 MB, noise, as the change allocates nothing.
S 363 → 364. Gate: 310 pass, 0 fail; full seeds 1–7 and 10–12 give the reference hashes; clippy
reports no warnings.

## Experiments that did not stay

- **The records written through a pointer** (local, AC power, turbo off, on step 52's code). The
  parse writes each record at `get_unchecked_mut(count)` and counts up. A pointer that moves up one
  record per line, with a closure that turns it back into a count for the runs and the final length,
  took 2 lines more and an `unsafe` write. Alone it saved 3.0 M instructions:u (591.5 → 588.5 M) and
  0.2 M cycles:u (324.3 → 324.1 M); on top of step 54's bound, 3.1 M instructions and 1.2 M cycles
  more (581.8 → 578.7 M and 322.3 → 321.1 M; `perf stat`, 30 interleaved runs). `ab.py`, 61
  interleaved runs, eleven rounds, step 52 / the bound / the bound and the pointer: medians
  18.86 / 18.79 / 18.70, 18.87 / 18.83 / 18.75, 18.85 / 18.75 / 18.75, 18.94 / 18.81 / 18.78,
  18.85 / 18.80 / 18.82, 19.00 / 18.84 / 18.69, 18.89 / 18.72 / 18.70, 18.95 / 18.82 / 18.85,
  18.96 / 18.81 / 18.82, 18.83 / 18.71 / 18.63 and 18.78 / 18.68 / 18.64 ms. On average the bound
  gained 0.11 ms (0.6%) for its line, and the pointer 0.04 ms (0.2%) more for its two, about what
  two lines cost.
- **Selection for every group, without the networks** (local, turbo off, on step 50's code):
  `percentiles` with only its three selections, and `sort_network`, `merge`, `stage` and their
  wildcard import deleted, S 379 → 334. `ab.py`, 61 interleaved runs: median 19.06 → 20.84 ms
  (+9.3%). `perf stat`, 4 interleaved rounds of 3 runs: instructions:u 598.9 → 649.8 M, cycles:u
  +12.8%. The 45 lines are worth about 3.6% of T. Not kept.
- **The count pass as one `flat_map` over the runs** (local, turbo off): 2 lines fewer than the
  nested loops. As a `for` over the `flat_map`, it ran 9.2 M instructions:u more (598.9 →
  608.1 M), because `next` checks for the end of the inner slice at every record. As `for_each`,
  which folds each run's slice in a loop of its own, it ran the same instructions but measured
  slower on step 53's code: `ab.py`, 101 interleaved runs, four rounds, medians 19.03 → 19.14,
  19.06 → 19.06, 18.77 → 18.87 and 18.99 → 19.02 ms (+0.3%), and +0.2% in eight rounds of 61
  runs beside step 51, more than the 0.16% its 2 lines are worth. Not kept.
- **Counts and errors in one `u64` per endpoint** (local, turbo off, on step 50's code): one
  read-modify-write per record in the count pass instead of two, with the errors in the upper
  half: 13.6 M instructions:u more, and no fewer cycles. **Unchecked indexing in the count pass
  and the scatter**: instructions:u −6.3%, cycles:u −1.1%, and no change in `ab.py`. Both passes
  wait on memory, not on instructions. Not kept.
- **Freeing more before the exit** (local, turbo off, on step 50's code). At `exit`, the kernel
  still unmaps 11.2 MB of output mapping, about 4 MB of heap, mostly the hours' lines in 12 malloc
  arenas, and 0.7 MB of page tables, on one thread inside the timed window: phase timestamps put
  0.75 ms between `exit` and the parent's `wait`, against about 0.2 ms for an idle 12-thread
  process. Alone, it unmaps about 99 ns per output page and 250 ns per heap page; 12 threads
  unmapping in parallel took 190 to 437 ns per page each, and every call interrupts the other
  threads to flush their TLBs. Releasing each hour's lines and its place in the output once it was
  formatted: medians +0.40 ms; its place only, +0.44 ms; its lines only, +0.18 ms. Each thread
  releasing its share of the output after its share of the input: −0.03 to −0.09 ms, about what
  its 3 to 4 lines cost; with the formatted lines released too, +0.07 to +0.11 ms. Finished
  threads spinning until the last one calls `exit`, instead of returning: +0.08 ms. The format
  queue in largest-first order: no change; with the finished hours' output released through a
  shared cursor as well: +0.04 to +0.06 ms. Not kept.
- **`put_number` inlined, with its eight- to ten-digit path in a `#[cold]` function** (local, turbo
  off, on step 49's code and on step 50's). `put_number` writes a value of eight digits or more by
  calling itself for the leading digits. That call keeps it out of line, and every call, however
  short its number, pushes and pops three registers and loads five 64-bit constants: 761,448 calls
  on seed 1, of which 1,836 have eight digits or more. With that path in a `#[cold]` `put_large`,
  `put_number` is inlined at its six uses in `format_hour`, and the constants stay in registers:
  instructions:u 616.0 → 602.8 M on step 49 and 598.9 → 585.8 M on step 50, but cycles:u medians
  337.8 → 337.9 M and 332.7 → 329.8 M (`perf stat`, 10 interleaved rounds of 3 runs). `ab.py`, 61
  interleaved runs, four rounds: medians 19.39 → 19.41, 19.34 → 19.35, 19.36 → 19.34 and
  19.34 → 19.37 ms on step 49, and 19.09 → 19.05, 19.08 → 18.95, 19.14 → 19.02 and 19.03 → 19.05 ms
  on step 50: −0.35% on average there and +0.05% on step 49, against the 0.3% that its 4 lines cost.
  Kept out of line by `#[inline(never)]` instead, it saved 7.6 M instructions:u on step 45 and no
  cycles:u (on battery, not comparable). Not kept.
- **Other selections for the groups over 128** (microbenchmark, one thread, ns per group for 129–256
  / 257–1,024 / over 1,024 values). The current chain: 1,035 / 2,087 / 8,655. A radix select on a
  log-scale key, a histogram and then the same chain on the candidate buckets: 1,109–1,422 /
  2,332–3,355 / 10,601–10,711. Two pivots from a 64-value sample around each rank, one pass that
  copies the values between them, and a select on those: 4,119 / 6,891 / 26,216, against 1,464 /
  2,997 / 12,365 for the chain in the same runs (on battery, not comparable). `sort_unstable` took
  13.3 ns per value against 4.6, and an AVX2 quickselect 2.53 ns per value against std's 2.21 for
  the median of the largest groups. A 256-value network took 803 ns against 1,038 for 129–256
  values: 0.35 ms of one thread's time over seed 1's 1,477 such groups, about 0.15% of T, less than
  its 2 lines cost. Not kept.
- **Other number formatting** (local, turbo off). Two digit groups in one `u128`: format
  1.60 → 2.22 ms per thread. Lines built straight in their place instead of on the stack: no change
  (microbenchmark, 41.4 against 41.3 ns per line). Name bytes from the padded copies: 0.1 ms per
  thread less, but `format_hour` then needs the interner, and the longer signature and copy cost 5
  lines after `rustfmt`. With `put_large` above, on step 45's code (on battery, not comparable): the
  digits split out of `eight_ascii` before the `'0'` offset is added saved 1.6 M instructions:u more
  for 2 more lines; a fixed 128-byte copy of the name, read past its end in the padded input
  mapping, saved 0.3 M instructions:u for 1 line; and the width as `2 + (value | 1).ilog10()` cost
  2 M instructions:u more. Not kept.
- **Slots of 16 bits** (local, turbo off, on step 45's code): an id needs 12, so `AtomicU16` slots
  halve the table to 16 KB. `ab.py`, 41 interleaved runs, two rounds: medians 20.25 → 20.44 and
  20.19 → 20.38 ms. Not kept.
- **Padded copies indexed by slot instead of by id** (battery, on step 45's code): a probe compares
  the window with the copy at its own slot, without loading the id first, and the hour pass maps
  ids to slots. The copies then take 512 KB, 128 pages, against 256 KB: DTLB misses that hit the
  STLB 1.9 → 4.8 M a run, and cycles:u +1.5–2%. Not kept.
- **The record written through a moving pointer** instead of at `records[count]` (battery, on an
  early form of step 46): 3.0 M instructions fewer, no fewer cycles, for 2 more lines. Not kept.
- **The status bit and the latency merged into one value before the lookup** (battery, on an early
  form of step 46): 6.3 M instructions more. Not kept.
- **Hours summarized during the parse, for a lower peak** (local, turbo off, on step 45's code).
  The log went out in 72 pieces of about an hour each, in file order. An hour was summarized and
  its records freed as soon as every piece from the one before its first line to the one after
  its last was parsed; a line arriving after its hour was done would have rerun the program
  without early hours (never tested). Each hour's lines were freed once it was formatted. On
  full seeds 1–7 and 10–12, 62 hours went early and the output matched. memory.peak on seed 1,
  5 alternating rounds: 17,444–18,280 → 15,112–15,672 KiB. The peak moved to the end of
  formatting (15.2 MB, 9.9 of it output), and the parse alone climbed steadily to 13.3 MB,
  because three things stayed: the 12 latency buffers, about 280 KB each and now touched during
  the parse, the lines of the hours done so far, and the pages that two runs share. `ab.py`, 61
  interleaved runs on seed 1, medians / minimums: 20.27 / 19.72 → 21.48 / 20.81 ms and 20.18 /
  19.71 → 21.48 / 21.02 ms (+6%). `perf stat -r 8`, twice: instructions:u 633.6 → 631.3 M,
  cycles:u 336–337 → 339–342 M, cycles:k 77–78 → 80–82 M, so the work grew about 2% and the
  rest was waiting. Phase timestamps: threads finished their last piece between 12.5 and 15.0 ms
  and waited for the last one, 0.8 ms each on average against 0.42 ms at the parse barrier, and
  with only the 10 hours left after the parse for 12 threads, each sat idle 0.56 ms on average,
  against 0.3 ms at the end of the full hour pass. S 361 → 461. Not kept.
- **Keeping each run's records until the hour pass ends** (local, turbo off, on step 45's code).
  Deleting `release(records)` after each run's scatter in `summarize_hour`, one line fewer, cost
  2.3%: `ab.py`, 61 interleaved runs, two rounds, medians 20.25 → 20.68 and 20.33 → 20.78 ms.
  The frees were not the cause. The work closure grew from 0x40ba to 0x40fb bytes and every
  spill offset moved, the parse loop's too, so cycles:u rose 2–4% (`perf stat -r 9`, twice:
  367–373 → 381–382 M) while cycles:k fell 5%. Keeping the call as `release(&records[..0])`
  frees nothing and leaves the closure's size and address as they were: `ab.py` medians
  20.25 → 19.96 and 20.08 → 19.93 ms (61 runs) and 20.17 → 20.14 and 20.39 → 20.03 ms (41 runs),
  0.8–1.5% faster, with cycles:k −2.7%. But memory.peak on seeds 1, 2, 3, 5, 7, 10 and 11, five
  rounds, went 18.15 → 23.06 MB (+4.9 MB): about a point lost, against 0.09–0.16 gained from T.
  Releasing only runs of at least 4 or 8 pages kept memory.peak within 0.16 MB (18.26 →
  18.42 MB, same seeds, four rounds), but it moved the closure again, to 0x4145 bytes: `ab.py`,
  41 runs, two rounds, medians +0.2 to +0.6 ms. Few of the calls free anything: seed 1 has 1,065
  runs, and only 222 of them hold a whole page, though those hold 98% of the records. For the
  others `madvise` gets no page and returns without interrupting the other threads. Not kept.
- **`#![no_main]` and a static interner, for an earlier start** (local, turbo off, on step 45's
  code). `#![no_main]`, with an `extern "C" fn main`, skips std's start-up, which reads
  `/proc/self/maps` and sets up signal stacks. The static interner kept its three tables in
  statics instead of on the heap. `ab.py`, 61 interleaved runs, four rounds, mean change of the
  medians: −0.02 ms for `#![no_main]`, 0 for the static interner and −0.06 ms for both. Phase
  timestamps (21 runs, two rounds) put `main` 94 µs sooner and the threads' median start
  0.16 ms sooner, but the last parse ended no sooner: 11.95 → 12.00 and 11.82 → 11.83 ms.
  Start-up savings barely reach the barrier: until a thread starts, its SMT sibling has the core
  to itself, so only about a quarter of a start delay shows at the barrier. Not kept.
- **A lock-free `insert`** (local, turbo off, on step 45's code): a new name took its id with a
  `fetch_add`, wrote its padded copy and its name, then claimed its slot with a compare-exchange
  and probed again if another thread had filled it first. The insert mutex costs a thread up to
  0.4 ms of waiting, about 3 ms summed over a run. Without it, context switches went from about
  340 to 118 (`perf stat -r 11`, twice) and cycles:k fell 3.5%, and phase timestamps put the
  parse's end 0.15–0.2 ms sooner. The wall did not follow: `ab.py`, 61 interleaved runs, four
  rounds, medians 20.25 → 20.26, 20.23 → 20.23, 20.13 → 20.12 and 20.23 → 20.28 ms. A timer
  that measures fork to wait as the board does, without `ab.py`'s extra `taskset` exec, gave
  19.11 → 19.18, 19.01 → 19.17 and 19.29 → 19.33 ms (61 runs, three rounds). Not kept.
- **No barrier before the output is mapped** (one line fewer: the first thread done with its
  hours waits for each hour's `OnceLock` instead). On step 39's PIE build, `ab.py` medians went
  20.92 → 20.44 and 20.80 → 20.31 ms, but phase timestamps put only 20 µs of that in the wait
  and 0.5 ms in the parse: `Barrier::wait`, left with one caller, was inlined into the work
  closure, and the parse loop inlined there came out with a different register allocation. On
  step 41's static build the same change cost 0.6 ms (medians 20.33 → 20.98 and 20.35 → 20.93
  ms). Not kept.
- **Line cuts that moved the hour pass's code** (local, `ab.py`, 40 runs, two rounds): an empty
  group skipped with `let [_, ..] = group else { continue }`, the hour's buffer used without
  re-slicing it to the hour, and the network's run lengths taken from a `for` over
  `(1..).map(|s| 1 << s)`. Each moved the network's code by 14 to 268 bytes, and each cost about
  3%: medians 19.98 → 20.52–20.58 and 19.93 → 20.53–20.61 ms. A fresh copy of the unchanged
  binary timed the same as the original (medians 20.20 → 20.28 and 21.06 → 20.57 ms), so it is
  the code's placement, not its page-cache copy. Setting the network's first partner distance
  with its run length, one line fewer, moved no permute but measured slower in all four numbers
  (medians 20.40 → 20.42 and 20.22 → 20.26 ms, minimums 19.68 → 19.80 and 19.79 → 19.83 ms), so
  it stayed out too.
- **PGO** (local, turbo off, on step 41's code): a profile from three runs on each of full seeds
  2–4, passed as `-C profile-use`, with every function matching it. `ab.py`, 61 interleaved runs,
  medians / minimums: 20.27 / 19.62 → 20.54 / 19.98 ms and 20.03 / 19.55 → 20.26 / 19.75 ms on
  seed 1; 20.18 / 19.70 → 20.48 / 20.00 ms and 20.34 / 19.75 → 20.66 / 20.13 ms on seed 10, which
  it was not trained on. `perf stat -r 5`, twice: instructions:u 690 → 662 M, cycles:u 359–361 →
  370–378 M, branch-misses:u 1.10 → 1.20 M.
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
- **Loops on 64-byte boundaries** (`-C llvm-args=-align-loops=64`, as track B builds; local,
  on step 45's code): `ab.py`, 40 interleaved runs, two rounds, medians / minimums 20.22 / 19.46
  → 20.87 / 20.22 ms and 20.37 / 19.61 → 20.99 / 20.35 ms. The output hash is unchanged.

## Profile notes

- Phase split after step 1b (native, turbo off): parse loop 18.3 ms per thread (finding line ends
  alone 6.9 ms, interning about 8, record push about 7), endpoint merge 1 ms, hour pass 13 ms,
  output write 4 ms, and about 8 ms outside `main`.
- Unmapping the input at exit, timed with an explicit `munmap`: 3.9 ms on one thread.
- `perf record` after step 2: parse loop 37% of cycles, `Interner::intern` 14%,
  `select_nth_unstable` 11%, hour-pass loop 9%, libc `memcmp`/`memcpy` about 9%.
