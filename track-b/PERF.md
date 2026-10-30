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
| 8 | Term table sized once from the queries' term count, and its keys trimmed after the intern | | | | |

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

## Experiments that did not stay

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
