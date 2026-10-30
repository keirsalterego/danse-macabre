# Track B: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 56.35 | 2200 | 0 |
| 1 | Rust v1: queries read first; one parallel pass keeps postings for query terms only and checks phrase windows at each phrase's trigger term | run-af10ba01e8f3 | 0.711 | 180.0 | 53.6 |
| 2 | Term keys read with one masked 32-byte load; tokens found 64 bytes at a time from AVX2 masks | run-03e880c5be0a | 0.5555 | 182.1 | 55.5 |
| 3 | Costly BOOL queries evaluated on per-chunk bitsets and id lists, the rest checked per document; one 16-byte prefetched probe per token; first matches grouped per thread | run-0ba4a60c4e07 | 0.4085 | 82.7 | 60.9 |
| 4 | Queries parsed on every thread into flat arrays; the plan's per-term lists built with one counting sort | run-03f5349811b3 | 0.3798 | 70.7 | 62.1 |
| 5 | Queries on sets evaluated every 4,096 documents inside the pass; glibc's mmap threshold fixed at 128 KiB | run-16bfddb1c5c1 | 0.3708 | 45.7 | 64.3 |
| 6 | Per-document BOOL queries compiled to 64-bit truth tables with one packed tally each, prefetched at the trigger term; term keys folded through a mask table | run-a7a48ad2968d | 0.2838 | 43.1 | 67.2 |
| 7 | Blocks of 2 MiB or more on transparent huge pages; a window's id lists hold 16-bit offsets | run-91f41d207c31 | 0.2793 | 46.3 | 67.0 |
| 8 | Term table sized once from the queries' term count, and its keys trimmed after the intern | run-533a441d6902 | 0.2705 | 45.1 | 67.5 |
| 9 | Tokens that are no query term take the id of a bit that is never set, so clearing the seen bits takes no branch | run-d85affefdce5 | 0.2679 | 44.2 | 67.7 |
| 10 | Only the term table on transparent huge pages, allocated alone at a 2 MiB boundary; the global allocator is gone | run-b4d6bfbdd0e9 | 0.2698 | 43.9 | 67.7 |
| 11 | A check's cell takes a match with a conditional move instead of a branch | run-6a27aa29cd80 | 0.2644 | 44.9 | 67.8 |
| 12 | The corpus walk takes one callback per token and one per line end, so the per-token one is inlined | run-68a53e20303e | 0.2450 | 43.8 | 68.8 |
| 13 | A token's hash multiplies the XOR of its key words once, instead of once per word | run-4a44efe39605 | 0.2403 | 43.4 | 69.0 |
| 14 | The per-document checks count a query's first matches in a byte per query, not in its cell | run-f56225b92327 | 0.2350 | 45.1 | 69.1 |
| 15 | Memory at its peak: a thread frees its window before grouping its first matches, and the grouping keeps no second copy of the group starts | run-7458013fd46b | 0.2346 | 42.3 | 69.2 |
| 16 | The query and check cells are written when allocated, so the pass's first store to a page no longer replaces the shared zero page and interrupts every core | run-6afbb627903d | 0.2343 | 42.0 | 69.2 |
| 17 | Each thread drops its chunk's pages of the corpus mapping when its pass ends, instead of the exit unmapping all of them on one core after the output is written | | | | |

A second run of the step 1 code, run-85b6361ddf94, measured 0.703 s and 181.1 MB.

Local numbers below come from 12 threads on the P-cores (CPUs 0-11, turbo off), full seed 1,
`perf stat -r 5`, with the variants interleaved over three rounds.

## Step 2: term lookup and tokenizer

- **Lookup:** term keys sit densely by id, and a token's key is one 32-byte load from the mapped
  file, masked to the token's length and lowercased with a shift, instead of a `memcpy` into a
  zeroed buffer that the next load then stalled on.
- **Tokenizer:** two AVX2 compares turn 64 bytes into a token-byte mask and an LF mask. Token
  starts and ends are `token & !prev` and `!token & prev`, walked with `trailing_zeros`, so no
  loop looks at single bytes. Locally: 0.632 → 0.579 s, cycles 14.60 → 13.27 G.

## Step 3: costly BOOL queries on sets, the rest per document

Seed 1 has 36.3 million tokens, and 79% of them are one of the 75,747 distinct query terms.
Step 2 appended every hit to a posting list (about 640,000 `Vec`s across the threads) and then
merged lists per query. The profile split evenly between the term lookup, the appends, and the
merges, and the lists set the 174 MB peak.

- **3a, two evaluation paths.** Each BOOL query gets a cover: terms such that every matching
  document holds one of them. If the cover holds a common term (one in at least 1 in 32 sampled
  documents), the pass records the query's terms as sets instead: a bit per document for common
  terms, an id list for rare ones. Those 1,772 queries are evaluated after the pass with
  word-wide AND, OR and AND-NOT, 64 documents per operation. The count is a popcount, and the id
  sum comes from six masked popcounts per word. The other 29,000 queries are checked at the end
  of each document that holds a cover term (17.4 million checks). Each check is a flat,
  self-contained program run against a per-document term bitset that fits in L1.
- **3a, one probe per token.** Each slot of the term table (16 bytes) holds a term's first 8 key
  bytes, its id, and flags saying what the pass does with it. A token thus costs one load, and
  only the rare term longer than 8 bytes reads the rest of its key. Each document's probes are
  prefetched as it is tokenized and resolved at its LF, so the loads overlap instead of
  stalling one by one.
- **3b, first matches.** Each thread groups its chunk's first ten matches per query with a
  counting sort over the match counts it already keeps. This replaced one serial sort of 1.25
  million (query, doc) pairs between the pass and the output.

Locally, step 2 → 3a → 3b: elapsed 0.584–0.592 → 0.469–0.534 → 0.427–0.429 s; cycles 13.28–13.56 →
9.97–10.08 → 10.04–10.09 G; memory.peak 173.3 → 82.1 → 79.7 MB. S went from 484 to 745.

## Step 4: the serial start

Before the pass, one thread parsed the queries and built the plan while the other eleven
waited: 56–59 ms of a 0.43 s run.

- **Parse, 32 → 15 ms.** Every thread parses a share of the query lines into flat arrays. One
  thread then gives each distinct term its id, in file order, so the ids do not depend on the
  thread count. It loads the table slot of the term 16 ahead while it interns one, so the
  cache misses overlap, and when the table doubles it is rebuilt from the keys in id order.
- **Plan, 25 → 15 ms.** A query's terms and operators sit in flat arrays instead of two `Vec`s
  per query. The phrases and checks each term triggers are collected as pairs and grouped by
  one counting sort, instead of a `Vec` per term.

Locally, step 3 → 4: elapsed 0.4335–0.4380 → 0.3905–0.3929 s; cycles 9.97–10.05 → 9.84–9.87 G;
memory.peak 79.2 → 69.2 MB. S went from 745 to 803.

## Step 5: memory

At step 4 the peak was 15 MB of tables and plan, plus about 4.3 MB on each of the 12 threads.
The biggest share was the bitset rows and id lists of the queries on sets, which each thread
kept for its whole chunk.

- **Sets per window.** Each thread keeps those rows and lists for one window of 4,096 documents,
  and evaluates the 1,772 queries on sets at the window's end. Their matches go to the same
  per-query tally as the other queries, so the output phase treats every query alike. The
  per-query "last matched document" array became a per-document list of matched phrases, and
  the term names are gone: a term's cost reads its length from its key. Local: memory.peak
  68.3 → 50.8 MB, cycles 9.91–9.97 → 9.85–9.91 G.
- **A fixed mmap threshold.** glibc raises its mmap threshold to the largest mapped block freed
  so far, and parsing and planning free blocks of up to a few MB, so smaller blocks then come
  from per-thread arenas and stay resident after they are freed. With
  `mallopt(M_MMAP_THRESHOLD, 128 KiB)`, every block of 128 KiB or more goes back to the kernel
  when it is freed, such as each thread's first matches once they are grouped. In the first
  measurement: 50.8–51.2 → 42.4–45.3 MB at the same speed.

Locally, step 4 → 5, six interleaved rounds on a busy machine: cycles 9.74–9.89 → 9.73–9.89 G,
elapsed 0.3926 → 0.3867 s in the quietest round; memory.peak 68.4–69.8 → 39.6–40.8 MB over five
runs. S went from 803 to 790.

## Step 6: checks without branches

Where step 5's time went on seed 1, found by switching off one stage at a time:

| Stage | ms |
|---|---|
| Start-up on one thread: query parse 15 (11 of it interning terms), plan 15 | 30 |
| Tokenizer | 41 |
| Each token's key: the u128 masking took about 77 instructions a token, and `key_in` was not inlined | 36 |
| Hashing, prefetching and storing each token's lookup for the line end | 19 |
| Term lookup at the line end (23 M branch misses) | 55 |
| Per-token bookkeeping, seen bits and flags (31 M branch misses) | 65 |
| Phrase checks | 7 |
| Per-document BOOL checks and their tallies: 17.5 M checks at about 160 cycles and 3 branch misses each | 132 |

Branch misses wasted 35% of all pipeline slots; waiting on memory, 5%. So the checks lost
their branches first. Each change below is measured on top of the ones before it (medians,
local, turbo off; step 5 ran 384–387 ms in the same sessions):

1. **Truth tables.** Each per-document BOOL query has at most 6 distinct terms, so it compiles to
   a 64-bit truth table: its AND, OR and NOT applied to six fixed bit masks. The skip for
   documents already counted from an earlier cover term is folded into the table. A check is a
   shift and a mask. 350.5 ms; branch misses 117 → 80 M.
2. **One tally per check.** Each check keeps its count and id sum in one u64 next to its
   neighbours, and the check and its tally are prefetched when the trigger term is first seen
   in a document. 320 ms.
3. **A mask table for the key fold.** A sliding table of 0xFF bytes replaces the u128 shifts,
   `key_in` inlines, and 2.2 G instructions disappear. 288.5 ms, but memory rose 8–10 MB.
4. **The memory back.** Each query keeps only its first ten matches, every per-query tally is
   one u64 (`count << 40 | id sum`), and the check tallies are freed before the grouping.
   296.4 and 296.9 ms over two rounds of 11 runs (min 289.6). memory.peak 44.9 MB against
   44.6 MB for step 5 (8 interleaved samples each, noise ±2 MB).

S 790 → 837. Gate: 507 pass, 0 fail. The packed sums limit the corpus to fewer than 2^20
documents; a larger one stops with an error, and the spec's full scale has 250,000.

Tried and dropped: a parallel sampling pass (3 ms faster, inside the noise), and no cap on the
first matches (5 ms faster, 2.7 MB more).

## Step 7: huge pages

At step 6 the pass spent 7.5% of its cycles walking page tables: 18.7 million dTLB load
misses walked to completion, 541 M cycles with a walk under way. Each token probes the 4 MiB
term table at a random slot, and a long token then reads its full key from a 2.4 MiB array.
On 4 KiB pages those tables span more pages than the TLB holds. The kernel here has
transparent huge pages set to `always`, but in the middle of a run only 2 MiB of the 18 MiB of
large blocks sat on huge pages: the blocks did not start on 2 MiB boundaries, and without
`MADV_HUGEPAGE` the kernel does not compact memory to find a huge page.

- **A global allocator** that gives every block of 2 MiB or more a 2 MiB alignment and calls
  `madvise(MADV_HUGEPAGE)` on it. Smaller blocks, and `realloc` between small sizes, still go
  straight to glibc, which grows a large block with `mremap` instead of copying it. Walks
  fell to 3.7 million and walk cycles to 142 M.
- **16-bit window lists.** A window spans 4,096 documents, so each document in one of its id
  lists ([`Window`]) fits in 16 bits as an offset from the window's first document. This
  halves the lists of the queries on sets on every thread, and it takes back about 1.1 MB of
  the memory the huge pages add.

Locally, step 6 → 7, three interleaved rounds: cycles 7.07–7.23 → 6.89–7.01 G, task-clock
3,059–3,131 → 2,958–3,014 ms, elapsed 0.2997–0.3045 → 0.2838–0.2843 s in the two quiet rounds.
Alternating runs, 20 each: median wall 295.2 → 285.3 ms. memory.peak over eight runs each:
41.1–44.2 → 40.2–45.1 MB (medians 41.9 and 43.3). S went from 837 to 871.

## Step 8: the term table sized once

The intern grew the term table from 1,024 slots by doubling, which rebuilt it eight times. The
table is now created with one slot per term occurrence in the queries, rounded up to a power of
two (about 153K occurrences and 75.7K distinct terms on all seven seeds, so 262,144 slots, the
size the doubling reached anyway). The intern drops from about 11 to 6.5 ms. The keys are trimmed
to their length once the intern ends, since the spare capacity would stay resident on huge pages.
The scan's machine code is unchanged.

Local, the time from start to the end of planning, medians of 6 interleaved runs: 27.96 → 23.5
ms. User instructions 10.23 → 10.06 G. memory.peak 40.2 → 40.6 MiB (noise about ±1 MiB). S
unchanged. Gate: 507 pass, 0 fail; full seeds 1–7 match.

What is left of start-up, and why it stays: query parsing 3 ms (already parallel), intern 6.3,
sampling 5.6, the plan loop 5.3, grouping and marking 3.2. A parallel sample, mark and plan loop
took start-up to 19.0 ms but added 55 lines, which costs more than it saves; a partitioned
parallel intern would take about 45 lines for about 4 ms.

## Step 9: one branch less per token

At each line end the pass clears the seen bit of every token's term. A token that is no query
term, 21% of them, had the id `u32::MAX` and was skipped with a test. That test mispredicted:
7.8 M of the 76.6 M branch misses. Such a token now takes the id of the bit that pads the
checks, which is never set, so clearing it changes nothing and the loop has no branch.

Locally, on step 7 (the scan is the same at step 8): branch misses 76.6 → 68.8 M; over three
interleaved rounds, cycles 7.07–7.10 → 6.81–6.84 G, task-clock 3,039–3,054 → 2,931–2,943 ms,
elapsed 0.2858–0.2864 → 0.2781–0.2791 s in the two quiet rounds. Alternating runs, 20 each:
median wall 286.0 → 277.3 ms (step 6: 297.9). S unchanged.

## Step 10: huge pages for the term table only

A huge page is resident as a whole, so step 7's allocator cost memory for every large block that
only partly fills its last huge page: on the board, 43.1 → 46.3 MB, and still 44.2 MB at step 9.
The term table is exactly two huge pages and the pass reads all of it, so it is the one block whose
huge pages cost no memory. It is now allocated alone at a 2 MiB boundary and advised once, and the
34-line global allocator goes, with step 8's trim of the keys, which only mattered on huge pages.
Walk cycles: 427–517 M with no huge pages, 136 M at step 9, 246–257 M now.

Locally, against step 9, three interleaved rounds: cycles 6.87–6.90 → 6.85–6.90 G, task-clock
2,952–2,967 → 2,969–2,989 ms, elapsed 0.2755–0.2770 → 0.2770–0.2791 s. Alternating runs, 14 each,
twice: median wall 275.6 and 276.9 → 276.8 and 278.7 ms (no huge pages at all: 282.5 and 282.2).
memory.peak over six runs each: 39.4–42.8 → 39.3–41.8 MiB; this machine does not show the
board's difference. S 871 → 849. Gate: 507 pass, 0 fail.

## Step 11: no branch on a check's result

A check adds `hit` times `1 << SUM_BITS | doc` to its cell ([`Tally::add_check`]), and the
compiler made that product a branch on `hit`. Whether a document matches a check follows no
pattern, so the branch missed 4.2 M times, 6% of all branch misses. `std::hint::select_unpredictable`
makes it a conditional move.

Locally, against step 10, three interleaved rounds: branch misses 68.2 → 64.0 M, cycles
6.80–6.84 → 6.61–6.68 G, task-clock 2,948–2,966 → 2,864–2,896 ms, instructions 10.06 → 10.22 G.
Alternating runs, 16 each, twice: median wall 278.2 and 276.3 → 270.1 and 269.2 ms. S unchanged.
Gate: 507 pass, 0 fail.

## Step 12: the per-token call inlined

[`walk`] reported tokens and line ends through one callback that took an enum. The callback held
the whole line-end logic as well, so the compiler kept it out of line and called it once per
token, 36.3 M times on seed 1: each call saved and restored six registers and reloaded the
captured state. `walk` now takes a token callback and a line-end callback that share a state
argument (the probes), so the token callback, a key, a hash and a prefetch, is inlined into the
loop. The `Event` enum goes.

Locally, against step 11, three interleaved rounds: instructions 10.22 → 8.86 G, cycles
6.60–6.67 → 6.16–6.25 G, task-clock 2,858–2,893 → 2,669–2,710 ms. Alternating runs, 16 each,
twice: median wall 272.0 and 270.5 → 255.4 and 254.0 ms (b-c4: 297.5 and 295.6). S 849 → 846.
Gate: 507 pass, 0 fail.

## Step 13: one multiply per token hash

The hash of a token's key folded its four words with a multiply each, a chain of four dependent
multiplies plus four extractions from the vector register, on every token. It now XORs the words
in the vector register and multiplies once. Equal XORs of different keys only share a probe
sequence, which the `head` compare still tells apart, and the slots still come from the top
bits of the product.

Locally, against step 12, three interleaved rounds: instructions 8.86 → 8.64 G, cycles
6.18–6.19 → 6.01–6.04 G, task-clock 2,677–2,681 → 2,609–2,623 ms. Alternating runs, 16 each,
twice: median wall 253.0 and 255.4 → 247.4 and 249.0 ms. S 846 → 844. Gate: 507 pass, 0 fail.

## Step 14: the first-match counts in bytes

A check's first ten matches in a chunk also go to its query's list of first matches, up to ten per
query, and the query's own cell counted them. That cell is one of 50,000 u64s per thread, 400 KB,
read at random on 7 M early hits, and mostly from L3. The count now sits in a byte per query
([`Tally`]), 50 KB per thread, which stays in cache. The query's cell is no longer touched during
the pass, so it no longer needs zeroing before the checks' cells are added in.

Locally, against step 13, three interleaved rounds: cycles 6.05–6.15 → 5.94–6.00 G, task-clock
2,623–2,669 → 2,582–2,607 ms. Alternating runs, 16 each, twice: median wall 247.9 and 247.3 →
245.4 and 244.8 ms. memory.peak over four runs: 40.2–44.1 → 41.7–42.5 MiB (the bytes add 0.6 MB
over 12 threads). S 844 → 845. Gate: 507 pass, 0 fail.

Not kept: each check pushing its first ten matches with no per-query count (format already takes
ten per query) gave the same time, 6 M more branch misses, and 4 MB more memory.

## Step 15: memory at the peak

Memory peaks when every thread groups its first matches at the end of its chunk: the ungrouped
(query, doc) pairs, about 122,000 per thread on seed 1 (976 KB), coexist with the grouped ids and
the group starts. Two things sat there for nothing. Each thread's window was still allocated,
its 349 bitset rows (179 KB) included, and [`group`] cloned its 50,001 group starts (200 KB) as
fill cursors. The window is now dropped after its last flush, and the grouping uses the starts
themselves as cursors, then shifts them back by one place.

Locally, memory.peak over seven runs each: 41.1–43.3 MiB (median 42.6) → 39.6–40.5 MiB
(median 39.9). The window alone: median 40.0, but one run at 42.8. Cycles and wall unchanged:
5.94–5.96 → 5.93–5.94 G; alternating runs, 12 each: median 246.0 → 245.1 ms. S 845 → 848.
Gate: 507 pass, 0 fail.

## Step 16: no zero page under the cells

perf's kernel profile of step 15 had 9% of kernel cycles in `smp_call_function_many_cond`,
called from `do_wp_page`: copy-on-write faults. `vec![0; n]` takes its cells from calloc, and
calloc leaves fresh pages from mmap unwritten. The pass first touches a page of query or check
cells with the load in `*cell += …` (load, add, store: the new value is compared next), so the
read fault maps the shared zero page. The store then faults again, and replacing a mapping that
other cores may cache sends an interrupt to every core running the process, to flush its TLB.
On seed 1 that happened for 1,111 pages of query cells and 948 pages of check cells over 12
threads, so 2,059 rounds of interrupts to 11 cores each. [`Tally::new`] now builds the cells with
`[0].repeat(n)`, which writes them, so each page faults once, on a write.

Locally, against step 15: page faults 18,800 → 16,710, kernel cycles 214 → 156–157 M, user
cycles unchanged (5.95–6.00 → 5.88–5.99 G). Alternating runs, 16 each, twice: median 246.4 and
246.1 → 244.9 and 243.4 ms, min 244.2 and 244.5 → 241.5 and 241.4 ms. memory.peak over seven
runs: median 39.8 → 39.9 MiB. S 848 → 848. Gate: 507 pass, 0 fail.

Not kept: the same writes as `black_box(&mut cells).fill(0)` after `vec!`, for the same time
(median 244.6 and 242.9 ms in the same runs) and 5 more lines. Also measured: recording first
matches in chunk 0 only, an upper bound on skipping the other chunks' first matches once chunk 0
holds ten for a query. Median 244.6 and 245.1 → 248.2 and 248.0 ms: chunk 0 still records and
groups all of its own, so its thread, the one the others wait for, does no less.

## Step 17: the corpus pages dropped in parallel

The kernel tears down the process's mappings in the exit, after the output is written, on one
core. For the corpus mapping that is 61,970 page table entries on seed 1 (254 MB), each a page
cache page whose map count, reference count and cgroup statistics it updates. Each thread now
drops its own chunk's pages with `madvise(MADV_DONTNEED)` as soon as its pass ends, so the
twelve cores share that work, mostly while other threads still scan. The range is rounded out to
whole pages: a neighbouring chunk that still reads a shared boundary page faults it back in from
the page cache, with the same bytes, as the mapping is private and never written.

Locally, against step 16, alternating runs, 16 each, three rounds: median 245.3, 242.8 and 243.5
→ 241.7, 241.3 and 240.4 ms; min 242.9, 241.6 and 241.1 → 239.7, 236.9 and 238.5 ms. With the
range rounded inward instead, to the chunk's own whole pages: 242.7 → 240.7 ms. Kernel cycles
rise a little, 171–182 → 178–192 M in a noisy session (the same work, spread over cores, plus
one TLB-flush interrupt per thread), but no longer at the end. memory.peak over seven
interleaved runs: median 40.0 → 40.1 MiB. S 848 → 852. Gate: 507 pass, 0 fail.

## Experiments that did not stay

- **A static-pie build** (`+crt-static`, as tracks A and C use), on step 14 (local, turbo off):
  `ab.py`, 16 interleaved runs, median / min 246.00 / 245.60 ms against 245.75 / 244.05 ms. The
  dynamic loader's quarter of a millisecond is lost in a 245 ms run.
- **Transparent huge pages off on top of the fixed threshold** (`prctl(PR_SET_THP_DISABLE)`):
  41.1–41.2 MB against 42.4–45.3 in the same session, but cycles 9.79–9.83 → 9.91–9.97 G and
  elapsed 0.383–0.385 → 0.392–0.397 s. Huge pages off without the fixed threshold gave
  49.8–50.8 MB, and `malloc_trim(0)` after planning 49.2–50.1 MB.
- **Every BOOL query checked per document, with no sets at all** (step 3 without the sets):
  1.29 s, 29.0 G cycles, 53.3 MB, against 0.63 s and 14.6 G for step 2. That meant about 100
  million evaluations, each chasing a query's heap-allocated terms and operators.
- **Step 3a's two paths with per-term arrays instead of table slots:** 15.1 G cycles, 0.64 s,
  76 MB, against 13.2 G and 0.58 s for step 2. With 75,747 terms, every hit touched about six
  random cache lines: the 1 MB slot array, 2.4 MB of keys, the last-seen array, the store array,
  and two 1.8 MB arrays of `Vec` headers. Moving the flags into the slots fixed that.
- **Term table grown at 3/4 load instead of 1/2** (2 MB of slots instead of 4): cycles 11.07 →
  11.53 G, elapsed 0.535 → 0.55 s, memory 81 → 77.7 MB. The longer probe runs cost more than the
  smaller table saved.
- **One per-document loop shared by the sample and the pass**, taking closures: fewer code
  lines, but cycles 10.0 → 10.6 G and instructions 12.25 → 13.60 G. A shared probe function
  that returned the slot cost 4% more cycles on its own. The pass keeps its loop inline.
- **A term table sized for every term occurrence up front** instead of doubling: parse
  13.8–14.7 → 10.7–11.0 ms, but 8 MB of slots instead of 4, as the queries hold 152,904 term
  occurrences and only 75,747 distinct terms.
- **Huge pages without the `realloc` path**, every resize a new block and a copy: median wall
  288.9 ms against 295.5 for step 6 (285.3 with it). The parse and the plan grow their arrays
  by copying. With `alloc_zeroed` also sent to glibc's `calloc` as well: 288.4 ms.
- **Huge pages only for blocks of 4 MiB or more** (the term table and its keys): the same
  cycles and memory as 2 MiB.
- **No branch per token at the line end, on top of step 6.** A token that is no query term
  resolved to an empty slot with a reserved id and no flags, and the trigger lists grew by
  writing each entry and advancing by 0 or 1. Branch misses fell from 77.6 to 65.1 M, but
  instructions rose from 10.07 to 10.57 G and cycles stayed at 7.19–7.20 G. Prefetching the
  triggered checks after the token loop, or at first sight behind one branch, was 0.5–1%
  slower than step 6.
- **The full keys of long tokens prefetched** in a pass between hashing and probing, on the
  step 5 code: cycles 8.40–8.44 → 9.09–9.12 G, as the extra pass stalls on the slot loads it
  reads. On step 6, skipping the long-key comparison altogether (wrong answers, as an upper
  bound) saved only 1.5–2.8% of cycles, too little for a prefetch scheme to chase.
- **Phrase triggers pushed without a branch** (each position pushed, then dropped again unless
  the term triggers phrases), on top of step 9: branch misses 68.8 → 67.2 M, but instructions
  10.06 → 10.44 G and cycles 6.80 → 6.87 G.
