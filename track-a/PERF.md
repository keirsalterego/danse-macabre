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
| 8 | Records in file order, 6 bytes per line, one entry per run of an hour; no hour sort | | | | |

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

## Experiments that did not stay

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
