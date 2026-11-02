# Track A: performance log

Every step I kept cites the live-board run that measured it. Numbers marked local are from my laptop (i7-13620H, turbo off), with the old and new versions run side by side, and the changes I measured but didn't keep are listed under the step they were tried after.

| | Reference (step 0) | Final code (step 64), two board runs |
|---|---|---|
| T | 32.05 s | 16.55 ms (run-3c87f8c35a0b), 16.87 ms (run-d83197d0cf00) |
| M | 228 MB | 16.9 MB, 17.0 MB |
| S (lines) | 56 | 344 |
| Points | 0 | 87.6, 87.3 |

## Checking the headline

Points are 50 · log10 K, where K = (Tref/T)^0.5 · min(Mref/M, 50)^0.2 · min(Sref/S, 1)^0.15 · (Dref/D)^0.15. For run-3c87f8c35a0b: (32.05 / 0.01655)^0.5 = 44.01, (228 / 16.9)^0.2 = 1.683, (56 / 344)^0.15 = 0.762, and the dependency factor is 1 because there are no crates. That makes K = 56.4, and 50 · log10 56.4 = 87.6, the board's number.

## What didn't work

- PGO (under step 41). A profile trained on seeds 2 to 4 cut instructions from 690 to 662 M and still made the run slower: 20.27 → 20.54 ms median on seed 1, and slower on seed 10 too, which it never saw.
- Handing out pieces from a shared counter instead of one chunk per thread. The median went 30.68 → 31.54 ms with 16 pieces per thread and 33.78 ms with 64. It only helped on battery power, when some threads started 4 to 8 ms late.

## How I worked

Nothing went to the board before it beat the old version locally in interleaved runs and passed all 320 cases of my gate. Lines cost points too: past the reference's 56, every 10% fewer lines is worth about as much as 3% less time, so I weighed each change's length against its speed before keeping it.

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
| 54 | The latency's space bounded with `assert_unchecked`, so reading the status and ending the name take no bounds checks: 9.8 M instructions fewer | run-c0db6c54d9a4 | 0.0225 | 18.0 | 83.8 |
| 55 | 3 fewer lines: `parse` an `unsafe fn` whose contract is the padding its `assert!` checked, and two range checks left to the indexes behind them | run-c0db6c54d9a4 | 0.0225 | 18.0 | 83.8 |
| 56 | 6 fewer lines at the same speed: a line's six numbers written in one `map`, the format queue built without collecting the hours, the latencies placed in the whole buffer, the two counters on one line | run-c0db6c54d9a4 | 0.0225 | 18.0 | 83.8 |
| 57 | Threads started one after another by one helper instead of by a tree, so that fewer of them wait for the interner's lock at once: 2 fewer lines | run-0899dd30b0f4 | 0.0172 | 18.4 | 86.7 |
| 58 | Each line's id settled one line late: its name's slot is loaded as soon as the hash is known, and the name compared with the slot's padded copy after the next line's front half | run-b601b34b577c | 0.0174 | 18.0 | 86.6 |
| 59 | The interner's pages given back once the names are sorted, before the hour pass's peak | run-b601b34b577c | 0.0174 | 18.0 | 86.6 |
| 60 | Each thread's share of the input released right after its parse, and the whole input unmapped by one thread once all parses are done, with the interner keeping copies of the names | run-8a3bcbee7e89 | 0.0194 | 17.3 | 85.6 |
| 61 | Records with an 11-bit endpoint id and a 20-bit latency: the list of large latencies holds 1.2% of seed 1's lines instead of 2.6% | run-6132d5943394 | 0.0164 | 17.5 | 87.3 |
| 62 | Each output line's endpoint id and request count in one `u32`, so the lines that wait for the formatting take 20 bytes instead of 24 | run-6132d5943394 | 0.0164 | 17.5 | 87.3 |
| 63 | 18 fewer lines: the interner's tables as four statics with `intern` and `insert` as plain functions, the network's seven merge levels through a one-line macro, and a Dockerfile without `WORKDIR` | run-f322558253fc | 0.0165 | 17.3 | 87.5 |
| 64 | 2 fewer lines: the leader releases the interner's three tables in one statement; the machine code is unchanged | run-4626d7369bff | 0.0167 | 17.5 | 87.3 |

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
3.5 to 5 ms per worker, more than the selection. Writing the digits directly into a small buffer
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
Peak memory is unchanged (24.2 to 24.9 MB against 23.9 to 25.1 MB). S 490 → 561. The static build was
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
818.8 M against 817.4 M, cycles +0.9%. memory.peak, 6 runs each: 22.4 to 23.1 MB against
22.7 to 23.8 MB. Gate: 307 pass, 0 fail.

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
plus 3 MB of lines). Local, turbo off: memory.peak 22.3 to 23.1 → 19.9 to 20.4 MB (8 runs each);
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
memory.peak 19.4 to 20.2 → 19.8 to 20.4 MB, the same within noise. S 425 → 410. Gate: 307 pass, 0
fail.

## Step 18: name copies on whole cache lines

The name table keeps a 64-byte padded copy of each name for the exact compare. malloc put that
array at 16 mod 64 on every thread, so each copy straddled two cache lines and every probe
loaded both, one of them as a split load. A `#[repr(align(64))]` wrapper puts each copy on one
line: L1 misses 9.6 → 8.1 M. The slots became a `Box<[u32; 8192]>`, so the mask is a constant
and the probe has no bounds check: user instructions 829.4 → 822.3 M.

Local, turbo off, `ab.py` with 31 interleaved runs, two rounds: medians 23.52 → 23.09 and
23.56 → 23.07 ms, minimums 23.16 → 22.64 and 22.98 → 22.72 ms. User cycles, 4 interleaved sets
of 5 runs: 448.6 → 441.5 M. memory.peak 18.95 to 19.88 → 18.53 to 19.90 MB. S 410 → 412. Gate: 307
pass, 0 fail.

## Step 19: one multiply per name hash

The name hash mixed three 8-byte windows of the name (first, middle and last) through three
dependent multiply-and-rotate steps. Now the windows are rotated apart (by 0, 21 and 42 bits),
XORed with the length and multiplied once by the same FxHash constant. The slot is the
product's top 13 bits, the bits every input bit reaches, which also drops the mask. In a
simulation of the 8,192-slot table on seeds 1, 3 and 5, 3.2% of lines need a second probe,
against 3.1% before (the old hash with top bits would need 3.8 to 7.6%). The table compares the
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
(slower than plain faults in a 12-thread test, 1.15 to 1.26 against 1.04 to 1.07 ms) or during the hour
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
Full seeds 1 to 4 have percentiles of 8, 9 and 10 digits, so the gate covers that path. Instructions
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

Instructions 774.28 → 770.72 M (-0.46%). Cycles of the network per call, rdtsc on an instrumented
copy, two runs: up to 16 latencies 125 → 117, up to 32 623 → 563, up to 64 1,414 → 1,230. The
percentiles and lines of an hour, timed per hour, 5 runs each: median 2.34 → 2.29 ms per thread.
That is about 0.06 to 0.1 ms per thread, under 0.5% of the run: user cycles 417.21 → 416.96 M over
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
0.16 to 0.21 ms of it sorting the names into output order. No hour needs that order before its
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
memory.current rose by 2.6 to 3.1 MiB to 19.4 to 20.5 MiB. The parse had topped out at 17.8 to 18.4 MiB
and the finished output at 19.3 to 19.5 MiB.

Two changes:

- Each worker's latency buffer is allocated once, with room for every line of the log
  (`vec![0; len / MIN_LINE + 1]`, 24 MB of address space). glibc's malloc maps a request that
  large as fresh zero pages, which `calloc` leaves untouched, so a page takes memory only once a
  latency lands in it.
- The hours go smallest first, so each buffer grows a little at a time while earlier hours free
  their records. The `threads` smallest hours go last instead, so every worker ends on a short
  hour, as with largest first.

In four runs of the same probe, the hour pass now tops out 0.2 to 1.2 MiB above where it starts, at
17.5 to 18.0 MiB, and the output at 16.8 to 17.2 MiB. The peak is the end of the parse, 17.6 to 18.3 MiB,
where the threads still parsing hold their name tables (about 190 KB each) on top of the
records. Local, turbo off: memory.peak, 20 alternating runs each, median 19.89 → 18.15 MiB on
seed 1; 10 each on seed 2, 19.97 → 17.96 MiB. `ab.py`, 61 interleaved runs, two rounds on seed
1: medians 22.56 → 22.33 and 22.72 → 22.53 ms, minimums 21.49 → 21.53 and 21.80 → 21.89 ms; one
round on seed 2: median 21.49 → 21.25 ms, minimum 20.98 → 20.89 ms. S 377 → 378. Gate: 310 pass,
0 fail (full seeds 1 to 7 and 10 to 12, 300 tiny seeds).

Tried and dropped, medians of memory.peak over 8 to 12 alternating runs, local, turbo off:

- The untouched buffer alone, largest first: 19.84 to 19.90 MiB. Most endpoints have a few
  latencies an hour, and their places are spread over the whole buffer, so its pages fill early
  anyway.
- Smallest first all the way: 17.91 to 18.04 MiB, but the longest hours then start last and the
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
from 17.6 to 18.3 to 16.2 to 16.4 MiB. The hour pass is now the peak, at 17.9 to 18.2 MiB: the per-thread
tables had left freed memory in the threads' malloc arenas, which the hour pass's small
allocations reused, and now those come from new pages. The output ends at 16.6 to 17.0 MiB.

Local, turbo off: memory.peak, 20 alternating runs, median 18.14 → 17.65 MiB on seed 1; 10 each
on seed 2, 17.91 → 17.81 MiB. `ab.py`, 61 interleaved runs, two rounds on seed 1: medians
22.28 → 21.54 and 22.20 → 21.42 ms, minimums 21.44 → 20.94 and 21.35 → 20.83 ms; one round on
seed 2: median 21.77 → 21.13 ms, minimum 21.30 → 20.49 ms. `perf stat` (10 runs, twice):
773.7 → 747.8 million user instructions, task clock 220.1 → 212.7 ms. The hour pass no longer
loads and bounds-checks an id for each record, twice, and the merge and the per-thread inserts
are gone. S 378 → 381. Gate: 310 pass, 0 fail (full seeds 1 to 7 and 10 to 12, 300 tiny seeds).
Seeds 1 to 3, 15 runs each on 12, 4 and 2 CPUs: all 135 outputs match the reference hash.

## Step 34: two buffers the allocator had filled

A copy of `/proc/self/smaps` at the barrier after the parse (local, seed 1, two runs) counted
13,312 to 13,324 KiB of anonymous memory, 12,144 KiB of it records. Two parts of the rest needed
no memory:

- The padded copies. `Box::new_zeroed_slice` gets a block aligned to 64 bytes from
  `posix_memalign` and then writes zeros over it, so all 256 KiB of the table were in memory,
  though the 1,842 copies fill 115 KiB. The table now holds `MaybeUninit` entries and comes from
  `Box::new_uninit_slice`, whose pages take memory only as copies are written: 260 → 116 KiB.
- The lists of large latencies. Each grew by doubling inside its thread's malloc arena, and
  every buffer it outgrew stayed there. Reserved like the records, with room for a large
  latency on every line, a list is a mapping of its own whose untouched pages cost nothing. The
  thread arenas went from 424 to 436 to 96 KiB, the main heap from 308 to 312 to 148 KiB.

At the barrier: 13,312 to 13,324 → 13,020 KiB. Local, turbo off: memory.peak, 20 alternating runs,
median 17.66 → 17.36 MiB on seed 1; 10 each on seed 2, 17.58 → 17.38 MiB. `ab.py`, 61
interleaved runs, two rounds on seed 1: medians 21.46 → 21.43 and 21.48 → 21.52 ms, minimums
21.01 → 21.02 and 21.02 → 21.05 ms. S 381 → 382. Gate: 310 pass, 0 fail (full seeds 1 to 7 and
10 to 12, 300 tiny seeds).

The peak is still the start of the hour pass. In the same probe as step 33, anonymous memory
rises by 1.1 MiB within 0.2 ms of it, as every worker writes its first latency buffer while
the records are all still there, and memory.current tops out 1.4 to 1.8 MiB above the end of the
parse. Tried and dropped, medians of memory.peak over 20 alternating runs, local, turbo off:

- Hours smallest first all the way, without moving the smallest to the end: 0.26 to 0.32 MiB
  lower, but `ab.py` median 21.50 → 21.72 ms (+1.0%, 61 runs).
- The 12 smallest hours first and the next 12 last: 0.14 MiB higher than the kept order.
- Each hour's lines released once formatted (1 line): in the probe the end of the output falls
  from 16.6 to 17.0 MiB (step 33) to 14.3 to 14.7 MiB, but the hour pass still sets the peak (17.47
  against 17.42 MiB without it).

## Step 35: two windows per name hash

A phase split of step 34 (instrumented build, 30 runs, local, turbo off; ms from before the
fork): `main` starts at 0.57, the first worker at 0.80 and the last at 1.33. The parse takes
11.1 to 11.7 ms per thread and ends between 12.39 and 12.64, the plan 0.07. The hour pass ends at
17.12 to 17.55: per thread, counting 0.60 ms, the scatter 1.10, releasing records 0.24, and
percentiles and lines 2.41. Formatting takes 1.67 ms per thread, releasing the input 0.59, the
last thread calls `exit` at 20.40, and the parent's `wait` returns 0.79 ms later.

`perf stat` counts 238.5 user instructions per line at 1.99 per cycle on each hyperthread, with
0.05 L2 misses per line and 1.4% of cycles stalled on an L1 miss: the parse is bound by
instructions, not by memory. The name hash read three 8-byte windows of the name, and the middle
one cost a subtraction, a shift, a bounds check, a load, a rotate and an XOR on every line. Now
it reads the first and the last only. In a simulation of the 8,192-slot table, 3.6 to 4.7% of lines
need a second probe instead of 3.1 to 3.4% (seeds 1, 3 and 5), each a mispredicted branch: far less
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
memory.peak, 11 alternating rounds on each of seeds 1 to 7 and 10 to 12: median 18.09 → 18.07 MB.
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
20.22 → 20.16 and 20.56 → 20.28 ms. memory.peak, 11 alternating rounds on each of seeds 1 to 7 and
10 to 12: median 18.08 → 18.13 MB, with no change to any allocation. S 380 → 381. Gate: 310 pass,
0 fail.
The board measured this code in the run of commit 3e78ba2, which changed only track C:
run-e44188ffe025 drew 19.7 ms, against 18.3 ms for step 36's code, a slow draw.

## Step 39: 5 fewer lines

The running sum that turns `summarize_hour`'s counts into ends is one `for_each` over the range
instead of a three-line `for` loop. `parse` lists a large latency and leaves `LARGE` in its place
with one `bool::then`, whose `mem::replace` hands the old value to the side list, instead of a
four-line `if`. `.text` and `.rodata` are byte-identical to step 38's, so T and M cannot move.
S 381 → 376. Gate: 310 pass, 0 fail.
The board's run, run-89adcd077219, drew 19.6 ms: the fourth draw of 19.5 to 19.7 ms in a row since
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
20.59 → 20.09 and 20.80 → 20.48 ms. memory.peak, 11 alternating rounds on each of seeds 1 to 7 and
10 to 12: median 18.18 → 18.21 MB. S unchanged at 376. Gate: 310 pass, 0 fail.

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
In the same push, track D's unchanged code drew 132.6 ms against its usual 122 to 128 ms: the box
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
all on the input: `L1-dcache-load-misses:u` on seed 1 was 4.27 M for step 45, and 6.72 to 6.74 M both
with `parse` out of line and with a change to `hour_index` alone, whose cycles:u rose 3.7 to 4.1%
though they ran about 6.4 M fewer instructions (battery, `perf stat`, 40 interleaved runs, two
rounds). Step 45's layout was the lucky one: the hardware prefetcher kept the input in L1 for it
and not for the others. Five changes:

- Each line prefetches the two cache lines from 384 bytes on (`_mm_prefetch` with `_MM_HINT_T0`),
  about five lines ahead. With them the misses are 3.85 M in every layout tried: 3.85 to 3.86 M in a
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
45: medians 20.33 → 19.73, 20.23 → 19.70, 20.21 → 19.70 and 20.25 → 19.74 ms (-2.5 to -3.0%),
minimums 19.67 → 19.12, 19.43 → 19.07, 19.63 → 19.18 and 19.50 → 19.04 ms. Timed by fork, exec and
wait, as the board's timer does, 41 runs, two rounds: medians 19.07 → 18.50 and 19.09 → 18.47 ms.
Without the static, the other four changes measured 19.81, 19.83, 19.83 and 19.78 ms in the same
rounds, and 18.66 and 18.66 ms by the second timer. Counters, measured on battery power, so only
the counts compare (`perf stat`, 40 interleaved runs, two rounds): instructions:u 690.9 → 662.6 M
(-4.1%), cycles:u medians 362.8 → 346.6 M and 363.0 → 347.1 M (-4.4%), minimums 351.2 → 326.9 M and
347.7 → 328.3 M, L1-dcache-load-misses:u 4.27 → 3.85 M. memory.peak, five alternating rounds on
seeds 1, 2, 3, 5, 7, 10 and 11: median 18.09 → 18.09 MB. S 361 → 367. Gate: 310 pass, 0 fail; full
seeds 1 to 7 and 10 to 12 give the reference hashes; clippy reports no warnings.

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
46: medians 19.73 → 19.54, 19.70 → 19.61, 19.70 → 19.63 and 19.74 → 19.54 ms (-0.4 to -1.0%),
minimums 19.12 → 18.92, 19.07 → 19.06, 19.18 → 19.08 and 19.04 → 18.91 ms; timed as the board does,
41 runs, two rounds, medians 18.50 → 18.46 and 18.47 → 18.47 ms. Counters, measured on battery
power, so only the counts compare (`perf stat`, 40 interleaved runs, two rounds): instructions:u
662.6 → 644.6 M (-2.7%), cycles:u medians 346.6 → 343.4 M and 347.1 → 342.9 M (-0.9 to -1.2%),
minimums 326.9 → 330.6 M and 328.3 → 313.7 M, L1-dcache-load-misses:u 3.85 → 3.85 M. A copy whose
`parse` an `#[inline(never)]` on `summarize_hour` moved 0x1b0 bytes on ran the same: cycles:u 344.5
and 342.2 M against 345.8 and 343.1 M in another session, L1 misses 3.85 M. memory.peak, as in step
46: median 18.09 → 18.00 MB. S 367 → 366. Gate: 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the
reference hashes; clippy reports no warnings.

## Step 48: an endpoint table of 16,384 slots

With 8,192 slots, 3.9% of seed 1's lines took a second probe, and each one is a mispredicted
branch. A simulation of the table, names inserted in file order, gives 1.9% with 16,384 slots:
4.20% → 2.27% on average over seeds 1 to 7 and 10 to 12, and at most 3.33% (seed 11). branch-misses:u
went 1.09 → 1.01 M a run (`perf stat`, 10 interleaved runs). The slots take 32 KB more of `.data`,
64 KB in all, and the L1 misses rose 3.85 → 3.93 M.

`ab.py` cannot tell this step from step 47. On AC power (local, P-cores, turbo off), 61 interleaved
runs, four rounds: medians 19.54 → 19.54, 19.61 → 19.60, 19.63 → 19.65 and 19.54 → 19.49 ms,
minimums 18.92 → 19.03, 19.06 → 18.90, 19.08 → 18.93 and 18.91 → 18.67 ms; timed as the board does,
41 runs, two rounds, medians 18.46 → 18.49 and 18.47 → 18.36 ms. Without the static interner of
step 46, the same change measured 19.72 → 19.53, 19.76 → 19.62, 19.75 → 19.52 and 19.65 → 19.58 ms
in the same rounds. Counters, measured on battery power, so only the counts compare (`perf stat`,
40 interleaved runs, two rounds): instructions:u 644.6 → 643.2 M (-0.2%), cycles:u medians
343.4 → 340.9 M and 342.9 → 341.7 M (-0.4 to -0.7%), and 345.8 → 342.1 M and 343.1 → 340.7 M in a
second session, minimums 330.6 → 327.2 M and 313.7 → 321.9 M. A copy moved as in step 47 ran the
same: cycles:u 342.1 and 340.3 M against 342.1 and 340.7 M, L1 misses 3.94 M. memory.peak: median
18.00 → 18.12 MB, and 18.40 → 18.26 MB in an earlier session. S 366 → 366. Gate: 310 pass, 0 fail;
full seeds 1 to 7 and 10 to 12 give the reference hashes; clippy reports no warnings.

## Step 49: three rare paths marked cold

`std::hint::cold_path()` marks a new run, a latency of 8 digits or more and a second probe. Without
the hints, LLVM's guess at how often each runs kept the loop's record count and its end on the
stack and rebuilt the LF pattern and two 64-bit constants on every line; with them, those stay in
registers, and the hot path has two taken branches fewer per line (br_inst_retired.near_taken:u
40.9 → 34.7 M): 26.0 M instructions fewer, 8.4 per line. Each hint alone, on an earlier form of
step 48: the second probe -16.7 M, the long latency -3.4 M, the new run -0.1 M; the three together
-26.0 M. Hints on a large latency, on a line of more than 103 bytes and on a name of 64 bytes or
more gained nothing.

On AC power (local, P-cores, turbo off), `ab.py`, 61 interleaved runs, four rounds, against step
48: medians 19.54 → 19.36, 19.60 → 19.41, 19.65 → 19.37 and 19.49 → 19.33 ms (-0.8 to -1.4%),
minimums 19.03 → 18.49, 18.90 → 18.82, 18.93 → 18.62 and 18.67 → 18.81 ms; timed as the board does,
41 runs, two rounds, medians 18.49 → 18.25 and 18.36 → 18.14 ms. A copy whose `parse` an
`#[inline(never)]` on `summarize_hour` moved 0x170 bytes on timed the same: medians 19.35, 19.40,
19.40 and 19.36 ms, and 18.16 and 18.31 ms. Counters, measured on battery power, so only the counts
compare (`perf stat`, 40 interleaved runs, two rounds): instructions:u 643.2 → 617.2 M (-4.0%),
cycles:u medians 340.9 → 334.1 M and 341.7 → 333.3 M (-2.0 to -2.5%), minimums 327.2 → 321.1 M and
321.9 → 318.0 M, L1-dcache-load-misses:u 3.93 → 3.94 M; the moved copy 333.8 and 333.6 M.
memory.peak: median 18.12 → 18.17 MB. S 366 → 369. Gate: 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12
give the reference hashes; clippy reports no warnings.

Steps 46 to 49 together, in the same rounds: medians 20.33 → 19.36, 20.23 → 19.41, 20.21 → 19.37
and 20.25 → 19.33 ms (-4.1 to -4.8%), and 19.07 → 18.25 and 19.09 → 18.14 ms timed as the board
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
thread, ns per group), 17 to 32 values went 99.5 → 52.7, 33 to 64 values 253 → 100, and 65 to 128 values 687
(selection) → 389 to 432; all groups 15.05 → 12.87 ms. As a loop, the 128-value network took 923 ns,
more than selection.

On step 45 the same change lost 2% (`ab.py`, 61 interleaved runs, medians 20.15 → 20.57 and
20.17 → 20.53 ms) although it ran 17 M instructions fewer: the parse was then inlined in the work
closure, and the larger closure gave it other registers and stack slots. With the parse out of line
in both builds, the networks took 2.5 to 2.9% fewer cycles:u. Since step 46 the parse is a function of
its own, and here its instructions are step 49's; only the address of the endpoint table in them
changes.

`ab.py`, 61 interleaved runs, four rounds, against step 49: medians 19.39 → 19.09, 19.34 → 19.08,
19.36 → 19.14 and 19.34 → 19.03 ms (-1.1 to -1.6%), minimums 18.71 → 18.32, 18.74 → 18.36,
18.93 → 18.52 and 18.76 → 18.43 ms; the first two rounds ran a build that also had `#[inline]` on
`merge` and `stage`, with a byte-identical `.text`. `perf stat`, 10 interleaved rounds of 3 runs:
instructions:u 616.0 → 598.9 M (-2.8%), cycles:u median 337.8 → 332.7 M (-1.5%). memory.peak, five
alternating rounds on seeds 1, 2, 3, 5, 7, 10 and 11: median 18.34 → 18.24 MB. S 369 → 379. Gate:
310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; clippy reports no warnings.

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
check. In a simulation of the 16,384-slot table, 1.9 to 2.5% of lines take a second probe instead of
1.9 to 2.3% (seeds 1, 3 and 5), and 2.2 to 3.0% with the CRC's low 14 bits. Seed 1's share grows most,
1.9 → 2.5%, and its branch-misses:u with it, by 0.02 M.

Local, AC power, turbo off. `perf stat`, 30 interleaved runs: instructions:u 600.9 → 591.7 M
(-1.5%), cycles:u median 328.8 → 324.1 M (-1.4%), branch-misses:u 0.898 → 0.916 M,
L1-dcache-load-misses:u 3.95 → 3.98 M. `ab.py`, 61 interleaved runs, four rounds: medians
19.01 → 18.84, 18.92 → 18.78, 19.06 → 18.85 and 19.24 → 19.20 ms; seven earlier rounds of builds
with the same `.text` as steps 51 and 52 (three of 41 runs, four of 61): 19.12 → 18.97,
19.11 → 18.86, 18.96 → 18.80, 19.03 → 18.96, 19.05 → 18.85, 19.03 → 18.87 and 19.62 → 19.04 ms.
memory.peak, three batches of alternating rounds on seeds 1, 2, 3, 5, 7, 10 and 11 (3, 8 and 10
rounds): medians 18.21 → 18.41, 18.27 → 18.37 and 18.26 → 18.16 MB, where two more builds with the
same `.text` as the two drew 18.24 and 18.21 MB: noise, as the hash allocates nothing. S 373 → 372.
Gate: 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; clippy reports no
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
19.11 → 19.01 ms (-0.3% on average), minimums 18.53 → 18.32, 18.39 → 18.35, 18.27 → 18.45,
18.41 → 18.37, 18.49 → 18.40, 18.65 → 18.54, 18.65 → 18.53 and 18.53 → 18.31 ms. memory.peak,
five alternating rounds on seeds 1, 2, 3, 5, 7, 10 and 11, three times: medians 18.17 → 18.28,
18.05 → 18.09 and 18.29 → 18.23 MB, from the same allocations. S 373 → 364. Gate: 310 pass,
0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; clippy reports no warnings.

## Step 54: the latency's space bounded

`latency` returns the index of the line's last space, and the parse reads the status at `space - 3`
and ends the name at `space - 4`. On the fast path the space lies 1 to 8 bytes below the LF, from
byte 31 on. The slow path, for 8 to 10 digits, now lifts it to at least 4, which only a malformed
line can need. `std::hint::assert_unchecked` hands the parse that range, `4..LINE`, and LLVM drops
the bounds checks of the status's byte and of the name's end: 3.1 instructions fewer per line.

Local, AC power, turbo off, on step 53's code. `perf stat`, 30 interleaved runs: instructions:u
595.2 → 585.4 M (-1.6%), cycles:u median 321.2 → 320.9 M, L1-dcache-load-misses:u 3.98 → 3.98 M.
`ab.py`, 61 interleaved runs, six rounds: medians 18.92 → 18.88, 18.80 → 18.77, 18.84 → 18.79,
18.89 → 18.79, 18.86 → 18.81 and 18.79 → 18.75 ms, -0.05 ms (-0.3%) on average. On step 52's code,
before step 53 went in, the same change ran instructions:u 591.5 → 581.8 M and cycles:u
324.3 → 322.3 M, and -0.11 ms (-0.6%) on average in eleven rounds of 61 runs (listed with the
pointer form under "Experiments that did not stay"). memory.peak, eight alternating rounds on seeds
1, 2, 3, 5, 7, 10 and 11: median 18.26 → 18.30 MB, noise, as the change allocates nothing.
S 363 → 364. Gate: 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; clippy
reports no warnings.

## Step 55: 3 fewer lines

`parse` becomes an `unsafe fn`. Its `# Safety` section states what its `assert!` checked: its lines
end at least `LINE` bytes before the end of the log, as the padding from `map_file` provides. The
one call says why that holds: every chunk ends by `len`, `LINE` bytes before the end.

Two checks of input the spec rules out go to the indexes behind them. `hour_index` no longer checks
the hour against the span: an hour of the span's days outside it now stops the run at the plan's
index into the hours instead of at the parse's `expect`. `Interner::insert` no longer asserts that a
4,097th name has no room: the index of its padded copy stops the run instead. Tried with hand-made
logs, each run ends with "index out of bounds" and writes no output: a line at 17:00 on the first
day, one at 18:00 on the last, and 4,100 distinct endpoints. A day outside the span still ends at
the `expect`.

Local, AC power, turbo off. `perf stat`, 30 interleaved runs, against step 54: instructions:u
585.39 → 585.41 M, cycles:u median 322.2 → 320.6 M. `ab.py`, 61 interleaved runs, fourteen rounds:
medians 18.76 → 18.78, 18.77 → 18.76, 18.80 → 18.80, 18.79 → 18.99, 18.70 → 18.88, 18.84 → 18.83,
18.85 → 18.88, 18.82 → 18.73, 18.84 → 18.78, 18.83 → 18.74, 18.80 → 18.73, 18.76 → 18.76 and
18.76 → 18.73 ms, and 23.94 → 23.67 ms in a round on a busy box: no change. `.text`, `.data` and
`.bss` keep their sizes, `.rodata` loses 96 bytes of panic messages, and no allocation changes.
S 364 → 361. Gate: 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; clippy
reports no warnings.

## Step 56: 6 fewer lines

- `format_hour` writes a line's six numbers with one `map` instead of a `for` loop: 2 lines, with
  the same machine code.
- The work closure's two counters come from one `[0; 2].map(AtomicUsize::new)` on the line that
  names `all` and `sorted`, with `finished` renamed `done` so that the line fits: 1 line.
- The format queue no longer collects the hours into a `Vec<&Hour>`. The output's length and
  each hour's place read the hour's `OnceLock` with `wait`, which returns at once, since every
  hour is done by the barrier: 1 line.
- `summarize_hour` builds each run's iterator over its large latencies in the `for` header:
  1 line, with the same machine code.
- `summarize_hour` places the latencies in the whole buffer instead of a slice of the hour's
  length: 1 line. On step 45's code this cost 3% (see "Line cuts that moved the hour pass's code"
  below); here, beside the other four cuts, `ab.py` medians went 18.83 → 18.90, 18.87 → 18.84,
  18.80 → 18.79 and 18.88 → 18.85 ms (61 interleaved runs, four rounds).

`perf stat`, 10 interleaved rounds of 3 runs: instructions:u 593.1 → 593.3 M, cycles:u median
327.2 → 325.8 M. `ab.py`, 61 interleaved runs, six rounds against step 53, two of them under
outside load: medians 18.74 → 18.66, 18.79 → 18.84, 18.51 → 18.55, 19.10 → 19.07,
18.81 → 18.72 and 18.82 → 18.81 ms (-0.1% on average). memory.peak, alternating rounds on
seeds 1, 2, 3, 5, 7, 10 and 11, medians: 18.24 → 18.17, 18.19 → 18.33 and 18.23 → 18.33 MB in
five rounds each, 18.29 → 18.28 and 18.32 → 18.28 MB in ten; the one allocation that changes is
the queue's 576-byte vector, which goes. S 361 → 355 (on step 53's code, 363 → 357). Gate: 310 pass, 0 fail; full seeds 1 to 7
and 10 to 12 give the reference hashes; clippy reports no warnings.

The board's run of steps 54 to 56 together, run-c0db6c54d9a4 (commit ace82cf), drew 22.5 ms, a
slow draw: locally, step 56's binary ran a median of 18.84 ms against step 53's 18.87 (`ab.py`, 82
interleaved runs), and in the same run track D's unchanged code drew 127.8 ms, against 122 to 127.
A second run of the same code, run-e31417916add (commit e7e0423, which changed only PERF.md files),
drew 17.65 ms, 18.6 MB and 86.3 points, and a third, run-2b07a12535b3 (commit 1534f22, which
changed only track B), 17.33 ms, 18.8 MB and 86.5 points, and a fourth, run-63683fcc0a53 (commit
f073734, which changed only track C), 17.26 ms, 18.2 MB and 86.7 points.

## Step 57: threads started by one helper

Since step 14 the threads started as a tree: each handed the upper half of its numbers to a new
thread until it kept one, so the last of twelve started after four spawns. Now thread 0 spawns one
helper, which spawns threads 2 to 11 one after another and then works as thread 1. `fan_out` goes,
and each thread gets its own copy of the work closure, which holds only references. With one
hardware thread, the helper spawns nothing and does no work.

The last thread starts no sooner: phase timestamps (41 runs, two rounds) put its start 1.38 and
1.45 ms after the fork with the tree and at 1.48 and 1.49 ms with the helper. But each thread's
parse took 8.53 and 8.51 ms instead of 8.91 and 8.86 (medians), and the last parse ended at 9.98 and
9.94 ms instead of 10.21 and 10.19. The difference is the interner's lock: a thread that finds it
taken spins briefly and then sleeps. A build that counts inserts found the lock taken at 700 and 712
of a run's 1,858 inserts with the tree, and at 336 and 248 of about 1,850 with the helper (means of
20 runs, twice). All inserts came in the first 1.4 ms, while the threads still met new names. With
the helper, thread 0 starts after one spawn instead of three and has the table to itself for about
0.3 ms, in which it adds about half the names; with the tree, three more threads started while it
had added fewer than a third. In two runs of a build that timestamps each thread's progress,
thread 0 took 0.62 and 1.19 ms over its first 4,096 lines with the tree, against 0.29 and 0.31 ms
with the helper.

Local, AC power, turbo off, on step 56's code. `perf stat -r 20`, twice: context switches 494 → 268
and 467 → 271. `perf stat`, 30 interleaved runs: instructions:u 585.53 → 585.52 M, cycles:u
321.4 → 318.5 M, cycles:k 113.6 → 112.0 M. `ab.py`, 61 interleaved runs, eight rounds: medians
18.71 → 18.48, 18.75 → 18.60, 18.67 → 18.38, 18.75 → 18.49, 18.59 → 18.39, 18.80 → 18.56,
18.83 → 18.52 and, under outside load, 23.30 → 22.91 ms: -0.24 ms (-1.3%) on average over the first
seven. The same code before its comment was written, whose panic messages name other lines, measured
-0.26 ms on average in eight more rounds. memory.peak, alternating rounds on seeds 1, 2, 3, 5, 7, 10
and 11, medians: 18.26 → 18.51, 18.40 → 18.48, 18.27 → 18.47 and 18.32 → 18.42 MB in four, six, ten
and ten rounds, and for the same code before its comment 18.18 → 18.18, 18.40 → 18.42 and
18.32 → 18.41 MB in three of them: about 0.1 MB more, worth about 0.2% of T, though every allocation
is the same. S 355 → 353. Gate: 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference
hashes; clippy reports no warnings.

## Step 58: each line's id settled one line late

A profile of step 57 (local, AC power, turbo off, seed 1; `perf record` of cycles, eight runs) put
43% of the cycles in `parse`, and 27% of its samples on the `vpmovmskb` right after the compare that
loads the slot's padded copy, 6% more on the test after the slot load. The two loads depend on each
other: the slot's index comes from the CRC, the copy's address from the slot. Precise samples of
`mem_load_retired.l1_miss` put an L1 miss on the slot load for 34% of lines and on the copy's two
halves for 25% and 20%, nearly all L2 hits (144,000 L2 misses a run in all). Each hyperthread has
half of the core's reorder buffer, about two and a half lines of this loop, so a line whose two
loads missed one after the other could not retire, and the lines behind it waited.

Now the loop loads the slot of a line's name as soon as the hash is known, and carries it to the
next iteration with the name, the 64 bytes from it, the slot's index and the record without its id.
That iteration first does the next line's front half (its LF, latency, status, name, hash and slot
load) and only then compares the carried name with the padded copy its slot names, and stores its
record. `intern` takes the slot and the content loaded from it instead of the hash; a second probe
loads the next slot as before. When `insert` finds that another thread filled the slot first, it
probes on from that slot with its new content instead of hashing the name again: the slots before
it on the name's path hold other names for good. So does the next line when it has the same new
name, since its slot was loaded before this thread filled it. The first line of a chunk has no line
before it to finish, and the last is finished after the loop.

Local, AC power, turbo off, seed 1. `perf stat`, three interleaved rounds of 10 runs: instructions:u
583.5 → 627.6 M (14 per line more), cycles:u 319.2, 322.3 and 319.9 → 307.1, 310.5 and 308.4 M
(-3.6%), cycles:k and branch-misses:u unchanged. Top-down (10 runs): retiring 54.5 → 59.8% of the
slots, backend-bound 22.2 → 18.0%. `perf record`: samples at the copy's compare 27 → 6% of the
parse's, after the slot load 6 → 3%. Phase timestamps (41 runs): each thread's parse took about
8.4 to 8.7 → 7.9 to 8.2 ms (per-thread medians), and the plan reached the threads at 11.22 to 11.27 →
10.90 to 10.97 ms after the fork. `ab.py`, 61 interleaved runs, seven rounds, the first four on the
same loop before `insert` lost its second hash: medians 18.53 → 18.09, 18.51 → 18.02,
18.97 → 18.55 and 19.01 → 18.69 ms, and under outside load 19.40 → 18.79, 19.47 → 18.91 and
19.41 → 19.03 ms: -0.46 ms (-2.4%) on average. memory.peak, three alternating rounds on seeds 1, 3
and 5: median 16.9 → 16.9 MB. S 353 → 361. Gate: 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give
the reference hashes; clippy reports no warnings.

## Step 59: the interner released after the name sort

Nothing reads the interner after the plan: the parse is over, the names' slices are copied into
the plan, and the sort is the last reader of the padded copies and the name table. The thread that
sorts now releases the whole static with `release`, about 0.2 MiB of touched pages (the 64 KiB of
slots, the 116 KiB of copies and the name table's 1,858 entries). The release lands about 0.2 ms
into the hour pass, before each worker's first hour has filled its buffer, which is when memory
peaks (step 34). One `madvise` call: one TLB shootdown for the other threads, which are counting.

Local, AC power, turbo off. memory.peak, alternating runs: seed 1, 20 and 15 rounds, means
16.77 → 16.59 and 16.69 → 16.51 MB (medians 16.75 → 16.65 and 16.6 → 16.5); seed 3, 15 rounds,
17.21 → 16.85 (median 17.2 → 16.8); seed 5, 15 rounds, 17.05 → 16.93 (median 17.0 → 17.0): about
-0.2 MB. `ab.py`, 41 interleaved runs, two rounds, under outside load (load average about 3.5):
medians 18.69 → 18.77 and 18.73 → 18.72 ms, minimums 18.15 → 18.26 and 18.11 → 18.06 ms. S 361 →
362. Gate: 310 pass, 0 fail; clippy reports no warnings.

## Step 60: the input unmapped once the parse is over

A profile of step 59 (local, turbo off, seed 1; `perf record -g` of user and kernel cycles, eight
runs) put 32% of all samples in the kernel: page faults 20% (the input's 6 to 8, the output's 6, the
records' 2), `madvise` 7.8% and the TLB-shootdown interrupts it sends 1.3%. Most of it is work per
4 KB page that the program cannot shrink: a 12-thread test that only maps the 239 MB file and
releases it spent 1.25 ms per thread on the faults and 0.6 ms on the release, whether the pages
were hot or cold. Two things around it were not fixed costs. The input's page tables, 0.4 MB of
kernel memory that `MADV_DONTNEED` leaves in place, stayed until the exit, so they counted in the
hour pass's peak. And the exit, on one thread inside the timed window, walked them and freed them.

Now each thread releases its share of the input right after its parse instead of after formatting,
and once every parse is done (the barrier) thread 1 unmaps the whole input with one `munmap`. Its
pages are gone by then, so the call only frees the page tables: 9 µs per thread on average, about
0.11 ms on thread 1, which the hour pass's shared queue balances out. Nothing reads the log after
the parse, because the interner now keeps a copy of each name (`Box::leak` in `insert`, 1,842
small allocations under the lock it already holds), which the sort and the formatting read.
`VmPTE` at the plan: 668 → 272 kB.

Phase timestamps (31 interleaved runs, medians): the last parse and its release end at 10.00 ms
after the start of `main`, against 9.31 ms for the parse alone (the release costs a little more
while other threads still parse); the last thread finishes formatting at 16.11 ms, where its
input's release used to end (16.13 ms); and the wall clock is 0.12 ms shorter, which leaves the
exit, no longer walking and freeing the input's page tables. Local, turbo off. `ab.py`, 61
interleaved runs: seed 1, medians 18.45 → 18.45 and 18.51 → 18.36 ms, minimums 18.13 → 17.73 and
17.99 → 17.94 ms (and 18.68 → 18.36 ms in a round of 41); seed 3, 18.63 → 18.48, 18.74 → 18.61
and 18.81 → 18.59 ms, minimums 17.68 → 17.81, 18.02 → 17.82 and 18.02 → 17.63 ms; seed 5,
19.01 → 18.85 and 19.01 → 18.75 ms, minimums 18.22 → 18.19 and 18.45 → 18.11 ms; seed 7,
18.62 → 18.53 and 18.63 → 18.56 ms, minimums 17.94 → 17.77 and 18.14 → 17.93 ms; seed 1 again,
18.37 → 18.30 ms: -0.15 ms (-0.8%) on average over the eleven rounds. `perf stat` (10 runs, twice, interleaved): instructions:u 627.7 → 631.3 M,
cycles:u 308.4 and 308.1 → 317.5 and 312.9 M, cycles:k 119.7 → 127.2 and 124.2 M, context
switches 264 and 284 → 426 and 368. memory.peak, alternating runs: seed 1, 16.60 → 16.27 MB (6
rounds) and 16.54 → 16.18 MB (8 rounds, the same code before its comments); seed 3, 17.02 → 16.48
MB (6 rounds); seed 5, 17.00 → 16.33 MB (6 rounds); seed 7, 16.55 → 16.37 MB (6 rounds): about
-0.4 MB. S 362 → 364. Gate: 310 pass,
0 fail; clippy reports no warnings.

## Step 61: a 20-bit latency field

What is live at the peak, on step 60's code (local, turbo off, seed 1). Copies that exit at chosen
points, timed by the same systemd-run memory.peak, 15 alternating rounds: 15.70 MB when every
parse is done, 16.09 MB after the first 12 hours, 16.22 MB for the whole run. A copy of
`/proc/self/smaps` when every parse is done: 13,216 kB of anonymous memory, 12,488 kB of it the
records (12,109 kB) and the lists of large latencies (about 314 kB), 244 kB the interner's `.data`;
`VmPTE` 676 kB, 272 kB after the unmap. With about 0.2 MB of kernel stacks that is 13.8 MB in use
against a peak of 15.7 MB: the rest is the kernel's charge batches, 64 pages at a time per
CPU, of which each of the twelve CPUs keeps the unused part, so memory.current reads up to 3 MiB
above the pages in use in every phase alike. The records are three quarters of the peak.

A record spent 12 bits on the endpoint's id, room for 4,096 names, but the spec fixes every seed's
catalogue at exactly 1,842 (§2), which 11 bits hold. The freed bit goes to the latency: with 20 bits
a record keeps every latency under 1.05 s, and the lists of large latencies shrink from 2.59 to
1.18% of the lines on seed 1, 4.42 to 2.09% on seed 3, 4.08 to 1.86% on seed 5 and 1.94 to 0.89% on
seed 7: 0.17, 0.28, 0.27 and 0.13 MiB fewer pages from the parse to the end of the hour pass. The
interner's names and padded copies, indexed by id, shrink to 2,048 entries (`.data` 426 → 249 KB,
the same pages touched), and a 2,049th name still stops the program at the index of its copy. Only
constants change, so S does not.

Local, turbo off. memory.peak, 5 alternating rounds each: seed 1 16.18 → 15.98 MB, seed 3 16.54 →
16.14, seed 5 16.50 → 16.08, seed 7 15.98 → 15.90: -0.28 MB (-1.7%) on average. `ab.py`, 61
interleaved runs: seed 1 medians 18.39 → 18.40 and 18.56 → 18.56 ms, seed 3 18.65 → 18.57 and
18.68 → 18.56 ms, seed 5 18.82 → 18.74 ms, seed 7 18.50 → 18.59 ms (-0.03 ms on average; a seventh
round under outside load left out). S 364 → 364. Gate: 310 pass, 0 fail; clippy reports no
warnings.

The same stop points after the change (seed 1, 10 rounds): 15.53 MB when every parse is done, 15.75
after 12 hours, 16.07 after 24 and 16.02 for the whole run. The peak is now the second round of
hours: each worker's buffer has grown to its second hour (29,000 to 39,700 lines, against 18,300 to
23,700 in the first), and the output lines of 24 hours, about 40 KB each, are held, while only the
first round's records have been freed.

## Step 62: 20-byte output lines

After step 61 the peak is the hour pass's second round, which holds the output lines of every hour
done so far. A line kept six `u32`: the endpoint's id, the request count, the three percentiles and
the error count. The id has 11 bits (step 61), and a count fits in the other 21: the most popular
endpoint draws about 12% of the 3.1 million lines (Zipf, §2), and seed 1's largest hour has 73,244.
So the two share one `u32`, and a line takes 20 bytes instead of 24: with 1,500 to 1,800 lines an
hour, about 0.15 MB less after 24 hours, and 0.5 MB less once all 72 wait for the formatting. The
code lines stay the same, with a shift and a mask.

Local, turbo off. memory.peak against step 61, two sets of 5 alternating rounds: seed 1 16.10 →
15.72 and 16.10 → 15.98 MB, seed 3 16.16 → 16.16 and 16.00 → 16.16, seed 5 16.14 → 15.94 and
16.18 → 15.90, seed 7 16.08 → 15.94 and 15.94 → 15.88: -0.13 MB (-0.8%) on average, nothing on seed
3. `ab.py`, 61 interleaved runs, nine rounds (the first four on the same code before a comment was
reflowed): seed 1 medians 18.59 → 18.83, 18.56 → 18.51 and 18.44 → 18.36 ms, seed 3 18.56 → 18.47
and 18.54 → 18.47, seed 5 18.60 → 18.56 and 18.70 → 18.72, seed 7 18.72 → 18.57 and 18.60 → 18.60:
-0.02 ms on average. S 364 → 364. Gate: 310 pass, 0 fail; clippy reports no warnings.

## Step 63: 18 fewer lines

- The interner's tables are four statics, `SLOTS`, `NAMES`, `PADDED` and `COUNT`, instead of the
  fields of one static `Interner`, and `intern` and `insert` are plain functions instead of its
  methods. The struct's definition, its initializer, which listed every field again, and the
  `impl` block go; `Padded` now holds the `UnsafeCell` and carries the `Sync` promise, and the
  leader releases the three tables one by one. Without `ENDPOINTS.`, the parse's `finish` closure
  fits on one line: 11 lines in all. `Cargo.toml` drops the `large_stack_arrays` exception, which
  clippy no longer needs, and whose comment named the interner's long-gone heap slots. The slots
  and the padded copies are all zeros, so they move from `.data` to `.bss` (`.data` 255 → 58 KB,
  `.bss` 24 → 220 KB). `parse` is the same machine code apart from the tables' addresses,
  `percentiles` and `put_number` are byte-identical, and the work closure differs in the leader's
  two extra `release` calls and in a few register and stack-slot choices.
- `sort_network` calls its seven merge levels through a local one-line macro,
  `merge_levels!(1 2 3 4 5 6 7)`: a level is a const generic argument, which a loop cannot supply.
  5 lines, and `.text` is byte-identical.
- `insert` tests the slot it loads with `if let value @ 1..` instead of binding it on a line of its
  own: 1 line, and `.text` is byte-identical.
- The Dockerfile's build stage runs `cd /src && cargo build` instead of setting `WORKDIR`: 1 line.
  Built in the pinned `rust` image, the binary is still a plain static executable (ELF type 2),
  so `.cargo/config.toml` still applies; with `--manifest-path` from `/` instead, it came out a
  static PIE (type 3).

Local, turbo off, against step 62. `perf stat`, 8 interleaved rounds of 5 runs, seed 1:
instructions:u 631.58 → 631.35 M, cycles:u median 313.2 → 311.9 M (6 of the 8 rounds lower).
`ab.py`, 61 interleaved runs, three rounds per seed: seed 1 medians 18.30 → 18.25, 18.36 → 18.31
and 18.36 → 18.34 ms (minimums 17.74 → 17.61, 17.77 → 17.64 and 17.86 → 17.82), seed 3
18.41 → 18.44, 18.39 → 18.32 and 18.50 → 18.45 ms (minimums 17.87 → 17.71, 17.83 → 17.81 and
17.83 → 17.92): -0.04 ms (-0.2%) on average. memory.peak, 5 alternating rounds: seed 1 median
15.8 → 15.8 MB, seed 3 16.2 → 15.9 MB. S 364 → 346 (Rust 358 → 341, Dockerfile 6 → 5). Gate:
310 pass, 0 fail; `cargo fmt --check` and clippy report nothing.

## Step 64: 2 fewer lines

- The leader releases `SLOTS`, `NAMES` and `PADDED` in one statement,
  `let _ = (release(&SLOTS), release(&NAMES), release(&PADDED));`, instead of three: rustfmt keeps
  the tuple on one line (52 characters, under its width of 60). `objdump -d` of the whole binary
  is identical to step 63's (`.text` 994,455 bytes), so T and M cannot move. S 346 → 344. Gate:
  310 pass, 0 fail; `cargo fmt --check` and clippy report nothing.

## Experiments that did not stay

- **`stage` folded into `merge`** (local, turbo off, on step 62's code), 3 lines fewer: with the
  stage's loop inside the level's, LLVM no longer unrolled every level for a constant `m`, and
  `percentiles` came out inlined into a `summarize_hour` of its own (19.6 KB, against 21.7 KB).
  `perf stat`, two interleaved rounds of 5 runs: instructions:u 631.3 → 649.5 M, cycles:u
  307.1 → 319.3 M. Not kept.
- **Line cuts that rustfmt undoes** (on step 62's code), each spread back over more lines than it
  saved: one `use` line for all of `std::sync` (nested braces go vertical), the parse's `Chunk`
  built as one struct literal (a literal's fields wider than 18 characters go vertical), the
  slot's load written into the tuple that carries a line to the next iteration, and the two
  prefetches as one `map` over `[384, 448]` (calls, tuples and chains wider than 60 characters go
  vertical; the bare `map` also trips `unused_must_use`).
- **Where the hour pass's excess comes from, and an in-place sort, not built** (on step 61's code,
  from seed 1's hour sizes and the stop points above). Smallest first in rounds of twelve, round k
  holds its buffers, about its hours' size S_k in records, and the lines of every hour done so far,
  less the records of the rounds before it: 250,049 lines in round 1, 416,539 in round 2 against
  250,049 freed, and about 40 KB of lines per hour on top. An in-place sort of each worker's first
  hour would leave round 2 as high as it is (at most 0.36 MB off); in place for every hour, about
  1 MB, it needs an in-place permutation across an hour's runs in different chunks and each large
  latency paired with its endpoint, tens of lines against 6% of M. Two scatter passes of half the
  endpoints each, to halve every buffer: one more read of every record, about the count pass's 3.5%
  of the samples, for 0.65 MB. Not built.
- **Pages that two finished runs share, released once both hours are done** (simulated on seed 1's
  runs in the kept order). A run's release frees only the pages wholly inside it, so a page shared
  with a run of another hour stays until formatting. Pages whose every run is done but which are
  still held: 21 after 12 hours, 72 after 24 (0.08 and 0.28 MiB), 291 after all 72 (1.14 MiB). At
  the peak, between 12 and 24 hours, under 0.3 MiB, for the bookkeeping of which neighbouring runs
  are done. Not built.
- **Each thread unmapping its own share of the input after its parse** (local, turbo off, on step
  59's code, names copied as in step 60). `munmap` holds the address space's lock, in read mode
  while it frees the pages, so the twelve calls queued behind each other: 2.8 ms per call on
  average, the last parse ended at 14.65 ms instead of 9.03 ms, and `ab.py` medians went
  18.37 → 22.79 and 18.39 → 22.73 ms (61 runs, two rounds), for 0.74 MB less memory.peak. With
  each share released by `MADV_DONTNEED` first, so that each `munmap` only frees page tables:
  `ab.py` median 18.55 → 18.70 and 18.58 → 18.75 ms (+0.9%), memory.peak -0.14 and -0.27 MB.
  Splitting the input's one mapping stalls the page faults of the threads still parsing it. Step
  60 unmaps once, after the barrier. The release after the parse alone, without any `munmap`:
  medians 18.44 → 18.41 and 18.58 → 18.49 ms, memory.peak unchanged. Not kept.
- **The input read with `pread` instead of mapped** (a 12-thread test in C, not built into the
  program): each thread reading its 20 MB share in 64 KB blocks into a buffer and touching every
  cache line took 6.87 ms; mapping it and touching every cache line took 6.3 ms, plus 0.6 ms to
  release it. Both stream at about 38 GB/s, so the faults are not the bound, and the copy would
  not overlap with the parse the way the mapped reads do. Not built.
- **The hour pass without its record releases** (local, turbo off, on step 59's code), to bound
  what batching them could save: `release(&records[..0])`, `ab.py` median 18.56 → 18.43 ms
  (-0.7%, 41 runs). The 222 calls that free pages each interrupt the other eleven threads, but the
  records of an hour lie in about three runs in different chunks, so they cannot be freed in fewer
  calls without the 4.9 MB that keeping them costs. Not built.
- **The chunk's records released before the output is mapped, not after** (local, turbo off, on
  step 60's code), so that eleven threads release while one maps: `ab.py` medians 18.51 → 18.50,
  18.57 → 18.44 and 18.58 → 18.60 ms (61 runs, three rounds). Not kept.
- **The latency buffer allocated before the first barrier instead of after the plan** (local, turbo
  off, on step 60's code), so that its `mmap` falls in the wait for the slowest parse rather than
  at the hour pass's start, where twelve such calls and the input's `munmap` queue for the address
  space's lock: `ab.py` medians 18.73 → 18.75 and 18.64 → 18.67 ms (61 runs, seeds 1 and 3). Not
  kept.
- **One malloc arena for all threads** (`mallopt(M_ARENA_MAX, 1)`, 2 lines, local, turbo off, on
  step 60's code), to spare the twelve arenas' `mmap`, `munmap` and `mprotect` calls, about 40
  sleeps on the address space's lock a run (`perf record` of context switches): `ab.py` medians
  18.47 → 19.16 and 18.61 → 19.30 ms (+3.7%), memory.peak 16.33 → 16.13 MB (6 rounds). Not kept.
- **Each hour's place in the output populated with `MADV_POPULATE_WRITE` before it is formatted**
  (local, turbo off, on step 60's code), to replace 35 page faults an hour with one call: `ab.py`
  medians 18.51 → 19.19 and 18.63 → 19.35 ms (+3.8%, 61 runs, seeds 1 and 3). Not kept.
- **The name's copy made before `insert` takes the lock** (local, turbo off, on step 60's code), so
  that the lock is not held through `malloc`: context switches 391 → 329 (`perf stat`, 10 runs),
  cycles:u unchanged, `ab.py` medians 18.44 → 18.41 and 18.65 → 18.59 ms. 1 line. Not kept.
- **`MADV_SEQUENTIAL` on the input** (local, turbo off, on step 60's code), so that its release
  skips marking each page accessed: cycles:k 124.9 → 122.8 M (`perf stat`, 10 runs), page faults
  unchanged, `ab.py` medians 18.62 → 18.58 and 18.65 → 18.71 ms. 1 line. Not kept.
- **Each hour's lines released once formatted, again** (local, turbo off, on step 60's code):
  memory.peak 16.22 → 16.17 MB (6 rounds). The hour pass's start still sets the peak. Not timed.
- **Where the kernel's time goes, at step 59** (`perf record -g`, eight runs): of the output's page
  faults (6.1% of all samples), 1.1% is threads spinning on the file's page-cache lock in
  `shmem_add_to_page_cache`, since all twelve add pages to one file at once; nothing in user space
  changes that short of fewer threads formatting. The input's faults spend 2.9% in
  `next_uptodate_folio`, which reads each page's `struct page` while it maps 16 pages per fault.
- **The input released once a thread's hours are done, before the second barrier** (local, turbo
  off, on step 58's code), to use the wait for the slowest worker: `ab.py`, 41 interleaved runs,
  median 18.81 → 19.66 ms (+4.5%). The release's shootdowns reach the workers still in the hour
  pass, and every name read while formatting faults its page in again. Not kept.
- **Each hour's whole output pages released once formatted** (local, turbo off, on step 59's code),
  so that the exit has fewer of the output's page-table entries to clear: `ab.py`, 61 interleaved
  runs, three rounds, medians 18.91 → 19.42, 18.81 → 19.05 and 18.96 → 19.29 ms (+1.9%), and 4
  lines more. The 72 `madvise` calls interrupt the formatting threads with shootdowns. Not kept.
- **Thread 0 spinning until every thread is done, instead of sleeping in the scope's join** (local,
  turbo off, on step 59's code), so that the exit need not wake it: `ab.py`, 41 interleaved runs,
  medians 18.74 → 18.63 and 18.66 → 18.84 ms. No difference beyond the noise, for 3 lines. Not
  kept.
- **`percentiles` inlined into the hour pass** (`#[inline(always)]`, local, turbo off, on step 58's
  code): instructions:u +0.8 M, cycles:u 309.3, 312.0 and 306.9 M against 305.3, 310.7 and 307.2 M.
  Not kept. In step 58's profile (5 runs, call graphs), `percentiles` is 8.9% of all samples, 5.9
  of them the selections for groups over 128 values, 2.5 the networks; the count and the scatter
  are 3.5 and 5.7%; page faults are 20.4% (input 5.8, the output's shmem pages 4.9, records and
  buffers 3.3, the rest entry and locking) and `madvise` 8.1%.
- **Other hour orders, simulated** (seed 1's hour sizes, time proportional to lines): the kept order
  ends 0.10 ms after largest first (4.35 against 4.25 ms per worker); ascending to the 30th hour,
  then descending, then the 12 smallest, ends at 4.28 ms. At most 0.4% of T, less than the run to
  run spread of the workers' last hour (0.7 ms between the first and the last worker, phase
  timestamps). Not built.
- **Other ways to settle the id late** (local, AC power, turbo off, on step 57's code; `perf stat`,
  10 runs, interleaved with step 58's loop where two numbers are given). The front half as a
  function that returns the whole next line in an `Option`, carried by `while let`: 712 M
  instructions:u and 322 M cycles:u against step 57's 583 and 318 M (as a closure, which LLVM did
  not inline, 772 M). The slot hashed again inside `intern` on every line: 665 M and 313 M. The slot
  only prefetched a line ahead and loaded just before the compare, `intern` unchanged: 646 M and
  310 to 312 M against 626 M and 306 to 308 M. The padded copy also prefetched once its slot was loaded:
  633 M and 309 to 310 M against 306 to 307 M. The status and the latency stored at once and the id ORed
  into the record a line later: 0.6 M instructions fewer, cycles:u 305 to 311 M against 306 to 309 M, and
  `ab.py` (41 runs) 18.22 against 18.12 ms. The slot hashed again only on the rare path, with
  `intern` taking the slot's content alone: 626.5 M, the same cycles, `intern` 7 lines longer, and
  `ab.py` medians 18.20 and 18.12 against 18.09 and 18.02 ms (61 runs, two rounds). Not kept.
- **16-bit slots, on step 58's loop** (local, turbo off): instructions:u +3.7 M, cycles:u 306.8 and
  309.5 M against 305.4 and 307.5 M (`perf stat`, 10 runs, interleaved). Not kept.
- **One input prefetch per line instead of two** (local, turbo off, on step 58's code): instructions:u
  -3.1 M, cycles:u 312 to 314 M against 306 to 310 M (`perf stat`, 10 runs, interleaved). Not kept.
- **Each thread's chunk mapped with `MADV_POPULATE_READ` before its parse** (local, turbo off, on
  step 58's loop): cycles:u -0.8%, but `ab.py`, 41 interleaved runs: median 17.99 → 18.62 ms
  (+3.5%), since each thread maps all of its 20 MB before it parses a line. Not kept.
- **The scatter's destination prefetched 16 records ahead** (local, turbo off, on step 58's code):
  instructions:u +31 M, cycles:u 308.0 and 307.5 M against 308.9 and 306.4 M. Not kept.
- **Measured, not built** (local, on step 58's code). About 2,800 TLB-shootdown interrupts reach
  CPUs 0 to 11 per run (`/proc/interrupts`), some 230 per thread, most of them from the hour pass's
  per-run record releases; step 45's notes put what dropping those releases saves at 0.8 to 1.5%, for
  4.9 MB more. `available_parallelism` takes 55 µs before the first spawn, reading the cgroup's CPU
  quota as well as the affinity mask; `sched_getaffinity` alone would start too many threads on a
  box that limits CPUs by quota. Creating and mapping the output takes 44 µs between the second
  barrier and the first hour's formatting. Per run, the last parse ends 0.15 ms after the mean of
  all twelve (10.73 against 10.58 ms, medians of 41 runs), which bounds any static reweighting of
  the chunks.
- **Other ways to start the threads** (local, AC power, turbo off, on step 56's code; `ab.py`, 61
  interleaved runs, medians against step 56). All eleven spawned by thread 0, 3 lines fewer than
  step 57: +0.24 to +0.45 ms in six rounds (+1.7%), since thread 0 began its own parse only after
  eleven spawns. A helper that only spawns, with thread 0 working meanwhile, so that 13 threads run
  for a moment, also 3 lines fewer: -0.24 to +0.57 ms in six rounds, -0.01 ms on average. The helper
  spawning threads 11 down to 2: -0.28 ms on average in eight rounds, and two helpers that spawn
  half of the threads each, 2 lines more: -0.26 ms, against -0.26 ms for step 57's helper in the
  same rounds, so the order made no difference beyond the noise. Not kept.
- **The interner's lock taken by spinning** (local, AC power, turbo off, on step 56's code):
  `try_lock` in a loop with `spin_loop` instead of `lock`, which spins briefly and then sleeps.
  `ab.py`, 61 interleaved runs, six rounds: medians +0.74 to +1.00 ms with the tree, and +0.05 to
  +0.16 ms with step 57's helper against the helper alone. Not kept.
- **The output written with `pwrite`** (local, AC power, turbo off, on step 56's code). With the
  file's size set as now, every thread opened the file for itself and wrote its hours through a
  `BufWriter` of 16 or 64 KB, seeking to each hour's place: no page faults on the output and nothing
  to unmap at exit, which phase timestamps put at 0.55 ms between `exit` and the parent's `wait`,
  against 0.74 and 0.79 ms. But tmpfs takes the file's lock for every write, so the threads' writes
  queued behind each other: formatting took 3.30 and 3.32 ms per thread instead of 1.60 and 1.62
  (64 KB buffers, 41 runs, two rounds). `ab.py`, 61 interleaved runs, six rounds: medians +1.43 to
  +1.62 ms with 64 KB buffers (+8.1%) and +1.63 to +2.13 ms with 16 KB (+9.4%). memory.peak, four
  alternating rounds on seeds 1, 2, 3, 5, 7, 10 and 11: median 18.18 → 18.39 MB. Not kept.
- **The latency's digits masked from the space itself** (local, AC power, turbo off, on step 55's
  code and on step 56's): `latency` keeps the digits above the last space with
  `0x0F0F_0F0F_0F0F_0F00 << (8 * last_space)` instead of
  `0x0F0F_0F0F_0F0F_0F0F << (8 * (last_space + 1))`: the mask's lowest byte is already zero, so the
  shift needs no `+ 1`. On step 55's code, one instruction fewer per line, instructions:u
  583.6 → 580.3 M, but cycles:u 318.3 → 320.7 M (`perf stat`, 40 interleaved runs). `ab.py`, 61
  interleaved runs, medians against step 55: +0.02, +0.02, 0, +0.03, -0.07, +0.02, -0.06, +0.03,
  +0.02 and -0.03 ms (ten rounds); against step 56: +0.07, -0.08, +0.18, -0.03, -0.03 and +0.23 ms
  (six rounds). Not kept.
- **The last hours handed out largest first** (local, AC power, turbo off, on step 57's code). The
  plan's order ends with the `threads` smallest hours, smallest first; reversed, 1 line more, a
  worker that frees up early takes the larger of them, as in largest-first scheduling. `ab.py`, 61
  interleaved runs, seven rounds and an eighth under outside load left out: medians -0.11 to
  +0.03 ms, -0.04 ms on average. Phase timestamps (41 runs, two rounds) put the hour pass, from the
  plan to the barrier, at 4.25 and 4.21 ms against 4.23 and 4.24, and the spread of the workers'
  last hour ends at 0.27 and 0.24 ms against 0.28 and 0.28: the tail is set by the size of the last
  hours, not their order. The last 24 hours largest first, 2 lines more: -0.01 to +0.03 ms. Not
  kept.
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
- **`fan_out` as a binomial tree** (local, turbo off, on step 53's code): thread `t` starts
  `t + 1`, `t + 2`, `t + 4` and so on above `2t`, which needs no ranges: 6 lines instead of 8.
  The last of twelve threads still starts after four spawns, but thread 0 makes four spawns
  before its own parse instead of three. `ab.py`, 61 interleaved runs, four rounds: medians
  18.92 → 18.97, 18.37 → 18.58, 18.78 → 18.89 and 18.80 → 18.91 ms (+0.6%). Not kept.
- **The network's functions without `#[target_feature]`**: `target-cpu=alderlake` enables AVX2
  for the whole crate, but rustc still counts a call to an AVX2 intrinsic as unsafe unless the
  calling function has the attribute itself ("the context requires the following additional
  target feature: avx2"), so the three attributes stay.
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
  and the scatter**: instructions:u -6.3%, cycles:u -1.1%, and no change in `ab.py`. Both passes
  wait on memory, not on instructions. Not kept.
- **Freeing more before the exit** (local, turbo off, on step 50's code). At `exit`, the kernel
  still unmaps 11.2 MB of output mapping, about 4 MB of heap, mostly the hours' lines in 12 malloc
  arenas, and 0.7 MB of page tables, on one thread inside the timed window: phase timestamps put
  0.75 ms between `exit` and the parent's `wait`, against about 0.2 ms for an idle 12-thread
  process. Alone, it unmaps about 99 ns per output page and 250 ns per heap page; 12 threads
  unmapping in parallel took 190 to 437 ns per page each, and every call interrupts the other
  threads to flush their TLBs. Releasing each hour's lines and its place in the output once it was
  formatted: medians +0.40 ms; its place only, +0.44 ms; its lines only, +0.18 ms. Each thread
  releasing its share of the output after its share of the input: -0.03 to -0.09 ms, about what
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
  on step 50: -0.35% on average there and +0.05% on step 49, against the 0.3% that its 4 lines cost.
  Kept out of line by `#[inline(never)]` instead, it saved 7.6 M instructions:u on step 45 and no
  cycles:u (on battery, not comparable). Not kept.
- **Other selections for the groups over 128** (microbenchmark, one thread, ns per group for 129 to 256
  / 257 to 1,024 / over 1,024 values). The current chain: 1,035 / 2,087 / 8,655. A radix select on a
  log-scale key, a histogram and then the same chain on the candidate buckets: 1,109 to 1,422 /
  2,332 to 3,355 / 10,601 to 10,711. Two pivots from a 64-value sample around each rank, one pass that
  copies the values between them, and a select on those: 4,119 / 6,891 / 26,216, against 1,464 /
  2,997 / 12,365 for the chain in the same runs (on battery, not comparable). `sort_unstable` took
  13.3 ns per value against 4.6, and an AVX2 quickselect 2.53 ns per value against std's 2.21 for
  the median of the largest groups. A 256-value network took 803 ns against 1,038 for 129 to 256
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
  STLB 1.9 → 4.8 M a run, and cycles:u +1.5 to 2%. Not kept.
- **The record written through a moving pointer** instead of at `records[count]` (battery, on an
  early form of step 46): 3.0 M instructions fewer, no fewer cycles, for 2 more lines. Not kept.
- **The status bit and the latency merged into one value before the lookup** (battery, on an early
  form of step 46): 6.3 M instructions more. Not kept.
- **Hours summarized during the parse, for a lower peak** (local, turbo off, on step 45's code).
  The log went out in 72 pieces of about an hour each, in file order. An hour was summarized and
  its records freed as soon as every piece from the one before its first line to the one after
  its last was parsed; a line arriving after its hour was done would have rerun the program
  without early hours (never tested). Each hour's lines were freed once it was formatted. On
  full seeds 1 to 7 and 10 to 12, 62 hours went early and the output matched. memory.peak on seed 1,
  5 alternating rounds: 17,444 to 18,280 → 15,112 to 15,672 KiB. The peak moved to the end of
  formatting (15.2 MB, 9.9 of it output), and the parse alone climbed steadily to 13.3 MB,
  because three things stayed: the 12 latency buffers, about 280 KB each and now touched during
  the parse, the lines of the hours done so far, and the pages that two runs share. `ab.py`, 61
  interleaved runs on seed 1, medians / minimums: 20.27 / 19.72 → 21.48 / 20.81 ms and 20.18 /
  19.71 → 21.48 / 21.02 ms (+6%). `perf stat -r 8`, twice: instructions:u 633.6 → 631.3 M,
  cycles:u 336 to 337 → 339 to 342 M, cycles:k 77 to 78 → 80 to 82 M, so the work grew about 2% and the
  rest was waiting. Phase timestamps: threads finished their last piece between 12.5 and 15.0 ms
  and waited for the last one, 0.8 ms each on average against 0.42 ms at the parse barrier, and
  with only the 10 hours left after the parse for 12 threads, each sat idle 0.56 ms on average,
  against 0.3 ms at the end of the full hour pass. S 361 → 461. Not kept.
- **Keeping each run's records until the hour pass ends** (local, turbo off, on step 45's code).
  Deleting `release(records)` after each run's scatter in `summarize_hour`, one line fewer, cost
  2.3%: `ab.py`, 61 interleaved runs, two rounds, medians 20.25 → 20.68 and 20.33 → 20.78 ms.
  The frees were not the cause. The work closure grew from 0x40ba to 0x40fb bytes and every
  spill offset moved, the parse loop's too, so cycles:u rose 2 to 4% (`perf stat -r 9`, twice:
  367 to 373 → 381 to 382 M) while cycles:k fell 5%. Keeping the call as `release(&records[..0])`
  frees nothing and leaves the closure's size and address as they were: `ab.py` medians
  20.25 → 19.96 and 20.08 → 19.93 ms (61 runs) and 20.17 → 20.14 and 20.39 → 20.03 ms (41 runs),
  0.8 to 1.5% faster, with cycles:k -2.7%. But memory.peak on seeds 1, 2, 3, 5, 7, 10 and 11, five
  rounds, went 18.15 → 23.06 MB (+4.9 MB): about a point lost, against 0.09 to 0.16 gained from T.
  Releasing only runs of at least 4 or 8 pages kept memory.peak within 0.16 MB (18.26 →
  18.42 MB, same seeds, four rounds), but it moved the closure again, to 0x4145 bytes: `ab.py`,
  41 runs, two rounds, medians +0.2 to +0.6 ms. Few of the calls free anything: seed 1 has 1,065
  runs, and only 222 of them hold a whole page, though those hold 98% of the records. For the
  others `madvise` gets no page and returns without interrupting the other threads. Not kept.
- **`#![no_main]` and a static interner, for an earlier start** (local, turbo off, on step 45's
  code). `#![no_main]`, with an `extern "C" fn main`, skips std's start-up, which reads
  `/proc/self/maps` and sets up signal stacks. The static interner kept its three tables in
  statics instead of on the heap. `ab.py`, 61 interleaved runs, four rounds, mean change of the
  medians: -0.02 ms for `#![no_main]`, 0 for the static interner and -0.06 ms for both. Phase
  timestamps (21 runs, two rounds) put `main` 94 µs sooner and the threads' median start
  0.16 ms sooner, but the last parse ended no sooner: 11.95 → 12.00 and 11.82 → 11.83 ms.
  Start-up savings barely reach the barrier: until a thread starts, its SMT sibling has the core
  to itself, so only about a quarter of a start delay shows at the barrier. Not kept.
- **A lock-free `insert`** (local, turbo off, on step 45's code): a new name took its id with a
  `fetch_add`, wrote its padded copy and its name, then claimed its slot with a compare-exchange
  and probed again if another thread had filled it first. The insert mutex costs a thread up to
  0.4 ms of waiting, about 3 ms summed over a run. Without it, context switches went from about
  340 to 118 (`perf stat -r 11`, twice) and cycles:k fell 3.5%, and phase timestamps put the
  parse's end 0.15 to 0.2 ms sooner. The wall did not follow: `ab.py`, 61 interleaved runs, four
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
  3%: medians 19.98 → 20.52 to 20.58 and 19.93 → 20.53 to 20.61 ms. A fresh copy of the unchanged
  binary timed the same as the original (medians 20.20 → 20.28 and 21.06 → 20.57 ms), so it is
  the code's placement, not its page-cache copy. Setting the network's first partner distance
  with its run length, one line fewer, moved no permute but measured slower in all four numbers
  (medians 20.40 → 20.42 and 20.22 → 20.26 ms, minimums 19.68 → 19.80 and 19.79 → 19.83 ms), so
  it stayed out too.
- **PGO** (local, turbo off, on step 41's code): a profile from three runs on each of full seeds
  2 to 4, passed as `-C profile-use`, with every function matching it. `ab.py`, 61 interleaved runs,
  medians / minimums: 20.27 / 19.62 → 20.54 / 19.98 ms and 20.03 / 19.55 → 20.26 / 19.75 ms on
  seed 1; 20.18 / 19.70 → 20.48 / 20.00 ms and 20.34 / 19.75 → 20.66 / 20.13 ms on seed 10, which
  it was not trained on. `perf stat -r 5`, twice: instructions:u 690 → 662 M, cycles:u 359 to 361 →
  370 to 378 M, branch-misses:u 1.10 → 1.20 M.
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
- **Where memory.peak comes from, at step 57** (local, seed 1, a copy that reads its cgroup's
  memory.current and memory.stat in a loop on a 13th CPU, five runs; systemd-run's peak for the
  unchanged binary: 16.7 to 17.2 MiB). memory.current moves in steps of 0.25 MiB, the 64 pages that
  the kernel charges at a time and keeps per CPU, so it reads up to 3 MiB above the pages in use.
  At the end of the parse it reads 14.8 to 15.8 MiB: 13,064 KiB of anonymous memory (12,144 of it
  records, 244 the interner's `.data`) and 1.07 MiB of kernel memory (0.70 page tables, mostly the
  input's). Within 0.4 ms of the hour pass's start it peaks at 16.8 to 17.4 MiB: each worker's first
  hour (18,300 to 23,700 lines) scatters into fresh buffer pages before any record is freed, about
  1 MiB in all, 1.0 to 1.6 MiB above the parse's end. Formatting ends at 15.8 to 16.8 MiB: the 10.1 MB
  output, 4.1 MiB of anonymous memory (2.9 of it the hours' lines) and the kernel's 1.1. So the
  records set a floor of about 15 to 15.8 MiB
  under any order of the work. The cheap cuts in reach: the 12 smallest hours first, about
  0.25 MiB off the spike, for a later tail; the interner's slots and padded copies released after
  the name sort, 0.18 MiB; each hour's lines released once formatted, which only matters once the
  spike is gone and cost 0.18 ms before. A counting sort holds its source and its destination at
  once, so removing the spike needs an in-place sort of each first hour. Not built.

- **Where step 63's time goes** (local, seed 1, a copy that stamps `CLOCK_MONOTONIC` and
  `ru_minflt` at each phase of each thread; `perf record -c 200000` with the samples binned by
  those stamps). See the profile notes. Every idea below aims at a piece of that timeline, and
  none reached 0.8%.
- **The latency buffers leaked** (no `munmap` when a thread ends; local, on step 63's code):
  `ab.py`, 61 interleaved runs, seed 1, medians 18.22 → 18.20 ms and 18.51 → 18.51 ms. The process
  exit unmaps them anyway, and the exit is on the critical path either way. Not kept.
- **Each place's output released after the format loop** (`MADV_DONTNEED` on the shared output
  mapping): 18.45 → 18.67 ms. The release sends TLB-shootdown IPIs to every thread still
  formatting. Not kept.
- **A final barrier, then each thread releasing 1/12 of the output** so the exit tears down a
  mostly empty mapping: the exit call to the parent's `wait` falls from 0.576 to 0.356 ms, but the
  12 simultaneous slice releases take 0.089 ms under `mmap_lock` and IPI contention, and the
  barrier waits for the slowest formatter. `ab.py`, 61 runs, four rounds: seed 1 18.47 → 18.34,
  18.40 → 18.44 and 18.51 → 18.38 ms, seed 3 18.50 → 18.35 ms (-0.09 ms, -0.5%, on average),
  for 4 to 5 more lines. Not kept.
- **The input released after the first barrier, the last releaser unmapping it** (instead of each
  thread releasing its share straight after its parse): 18.43 → 18.56 and 18.39 → 18.61 ms. A
  release right after a parse overlaps its SMT sibling's parse (kernel work against compute);
  12 releases at once contend on the same locks. Not kept.
- **128-byte padded copies in the interner** (so names up to 127 bytes compare without `memcmp`):
  cycles:u +1% in interleaved `perf stat`; the extra 64 bytes per line cost more than the rare
  long-name `memcmp` saves. Not kept.
- **Two 64-byte tables for long names** (the second 64 bytes of a name in a table of its own):
  instructions:u -12.6 M, cycles:u -0.8%, but `ab.py` seed 1 18.49 → 18.45 ms and seed 3
  18.38 → 18.48 ms, for 4 more lines. Not kept.
- **Each thread's latency buffer allocated before its parse** (its `mmap` away from the input's
  `munmap` and the shared release): seed 3 18.62 → 18.76 ms. Not kept.
- **Not built, bounded by the timeline:** stealing release work from the slowest parser (all of
  the first barrier's mean wait is 0.157 ms), mapping the output before the hour pass (map_output
  takes 0.05 ms), and a SIMD `put_number` (the formatting's user time is 1.2 ms of the 1.55 ms
  phase; the rest is the kernel's shmem faults and `xa_lock`).

- **Two of the step 57 notes' memory cuts, after step 64** (local, turbo off). (1) The
  interner's three tables released before the names' output order is sorted, the sort comparing
  the names the plan already holds instead of the padded copies; (2) the hour order's last
  `2 × threads` hours reversed instead of the `threads` smallest moved to the end. memory.peak,
  6 alternating rounds on seeds 1, 3 and 5: median 15.90 MB for step 64, 15.95 with both cuts,
  16.00 with a third that wrote the output from the buffer's end. Within the noise of the
  kernel's charge batches; not kept.

## Profile notes

- Phase split after step 1b (native, turbo off): parse loop 18.3 ms per thread (finding line ends
  alone 6.9 ms, interning about 8, record push about 7), endpoint merge 1 ms, hour pass 13 ms,
  output write 4 ms, and about 8 ms outside `main`.
- Unmapping the input at exit, timed with an explicit `munmap`: 3.9 ms on one thread.
- `perf record` after step 2: parse loop 37% of cycles, `Interner::intern` 14%,
  `select_nth_unstable` 11%, hour-pass loop 9%, libc `memcmp`/`memcpy` about 9%.
- Timeline of step 63 (local, seed 1, a stamped copy, per-thread medians in ms from `main`'s
  start, 12 threads on 0-11): parse 8.6 (spread 0.40 across threads), share released 0.7, wait
  at the first barrier 0.157 on average, the leader's plan 0.06 (it ends 0.006 after the last
  release), unmap and latency buffer about 0.1, hour pass 4.0 (threads end within 0.29 of each
  other), latencies released, wait at the second barrier 0.141, `map_output` 0.05, `relchk`
  0.04, formatting 1.55 (end spread 0.07), then the exit call to the parent's `wait` 0.55.
  Kernel share of each phase (perf, cycles per run): parse 26% (`next_uptodate_folio`,
  fault-around), share release 100% (`unmap_page_range`), hour pass 21% (TLB-shootdown IPIs from
  the other threads' releases, first-touch faults), output placement 99% (IPIs, spinlocks),
  formatting 47% (`shmem` faults, `xa_lock` contention in `shmem_add_to_page_cache`), exit 60 to 70%
  (`osq_lock`, IPIs). The waits at the two barriers and the leader's serial plan add up to 0.36 ms
  (2.2%); the largest pieces left are the parse's user time and its fault-around.
