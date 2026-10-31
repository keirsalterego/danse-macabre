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
| 17 | Each thread drops its chunk's pages of the corpus mapping when its pass ends, instead of the exit unmapping all of them on one core after the output is written | run-bfce9886b69d | 0.2354 | 43.4 | 69.2 |
| 18 | A term table slot holds a term's first 12 bytes instead of 8, so a token of 9 to 12 bytes no longer loads its term's key from a second array | run-e847cb4a493b | 0.2208 | 41.8 | 69.9 |
| 19 | 99 fewer code lines: the cold code and the term table simplified (no doubling), the walk's bit pops in one helper; the key-mask table aligned to a cache line | run-74f2276732d6 | 0.2241 | 42.9 | 70.1 |
| 20 | The rarity sample is the corpus's first 1/64 (4,005 documents on seed 1) instead of its first 1,000 documents, each thread counting a part | run-75d28a617cc4 | 0.2158 | 42.8 | 70.5 |
| 21 | 35 fewer code lines: one helper runs and joins each set of threads, the intern loop drops its prefetch, the output adds each query's cells once; every loop starts on a 64-byte boundary | run-f0c533c66a88 | 0.2165 | 42.9 | 70.6 |
| 22 | 8 fewer code lines in code that runs once, with the same machine code | run-59d37d7b5803 | 0.2177 | 42.3 | 70.5 |
| 23 | A per-document check reads its terms' bits with one AVX2 gather instead of six loads, and the document's term bits are read and cleared without bounds checks | run-29bb809aa3f8 | 0.2048 | 43.5 | 71.2 |
| 24 | A document's checks are listed one by one as its BOOL triggers appear, so one loop runs them all instead of one loop per trigger | run-170e5552bd10 | 0.2085 | 42.4 | 71.0 |
| 25 | The token loop takes no branch on what a token is: a token that is no query term probes to an empty slot, id 0, and a document's new stored terms and BOOL triggers are listed by a store and an add | run-ab97ce51555f | 0.1746 | 43.5 | 72.9 |
| 26 | A document's term bits are cleared by zeroing their words, a store with no load, and the probe's end test is kept one branch by an empty asm block instead of `black_box`, which sent it through the stack | run-1d7859f315a0 | 0.1682 | 43.2 | 73.3 |
| 27 | Every thread plans a share of the queries, and the term table's flags are added to every slot, an empty one adding 0, without a test for empty slots | run-fc3b46db2437 | 0.1650 | 41.7 | 73.5 |
| 28 | The rarity sample probes a line's tokens at its end, as the pass does, and counts without a branch; interning a term takes no branch on whether it is new; the sample's end is found without the other 63 cuts | run-75ef64ef4099 | 0.1643 | 42.2 | 73.5 |
| 29 | A query on sets adds its window's matches in one vectorized sum instead of a call per word; a term's store is a `u32` code instead of an enum; the terms kept in rows and in lists are listed apart, so adding them takes no branch on which | run-575e79713710 | 0.1623 | 43.1 | 73.7 |
| 30 | A plain static executable (`crt-static` and `relocation-model=static`) instead of a dynamically linked PIE | run-4654d088e3bb | 0.1615 | 41.1 | 73.7 |
| 31 | The token loop writes a document's term ids and its kept rows, lists and BOOL triggers into four runs of one slice, and its seen bits, without bounds checks, so its registers no longer spill | run-ea58cfb946e5 | 0.1483 | 42.1 | 74.6 |
| 32 | 1 fewer line with the same binary: the build stage mounts its sources instead of copying them | run-ced2f9ec7167 | 0.1491 | 41.1 | 74.6 |
| 33 | A document's checks are added in one `Tally` method that takes no branch on a check's result: the queries of the checks that the document is one of the first matches of are moved to the front of its list of checks, and only those go on to the first-match counts | run-94e224675c5a | 0.1455 | 41.3 | 74.8 |
| 34 | The term lookups take no bounds checks: the probe reads the slots unchecked from a key's home slot on, the prefetches take pointers, and the token loop holds the term table and the seen bits as locals | run-401b90111691 | 0.1618 | 40.5 | 73.7 |
| 35 | 11% fewer instructions in the corpus pass: a document's checks are written to a slice sized once and evaluated without bounds checks, the tokenizer picks the start of a token carried over from the previous block once per block, a key's window takes one bounds check and its hash two words, phrases are compared inline, and a long key's mark rides in its slot's tail | run-8bf61f0fec95 | 0.1313 | 43.3 | 75.9 |
| 36 | The serial start about 2.2 ms shorter: the intern prefetches 16 keys ahead and keeps each term's length and only the long terms' keys, a term's cost is computed where the planning reads it, the sample keeps each term's count and last document in one cell, and the term table's flags are collected in bytes | run-ce4c2e1d13c6 | 0.1342 | 42.8 | 75.7 |
| 37 | The term table's huge pages asked for again: once the table is written, `MADV_COLLAPSE` collapses its 4 KiB pages into huge pages if its faults found no free one | run-5de0cb146d0e | 0.1445 | 41.4 | 74.8 |
| 38 | No frees once the output is formatted, as the plan and the chunks are `ManuallyDrop`, and the output's numbers written by `format_into` instead of `write!` | run-5de0cb146d0e | 0.1445 | 41.4 | 74.8 |
| 39 | The per-document checks on huge pages of their own, as the term table's slots are, through one helper; the rarity sample summed as each thread's part comes in instead of after the last; the plan's counting sort puts its items in from the back, so the starts need no rotation | run-8d0c750cfc9c | 0.1277 | 42.9 | 76.2 |

A second run of the step 1 code, run-85b6361ddf94, measured 0.703 s and 181.1 MB.

Step 24's board run drew 208.5 ms, 1.8% slower than step 23's 204.8 ms, although step 24 ran 2%
faster locally in three rounds (its section has them). A second run of the step 24 code,
run-6280980d331c, measured 202.0 ms and 43.5 MB (71.3 points). Draws of the same code vary on the board:
step 21's machine code measured 216.5 and 217.7 ms (rows 21 and 22).

Local numbers below come from 12 threads on the P-cores (CPUs 0-11, turbo off), full seed 1,
`perf stat -r 5`, with the variants interleaved over three rounds, unless a section says
otherwise.

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
term table at a random slot, and a long token then reads its full key from a 2.4 MB array.
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
ms. User instructions 10.23 → 10.06 G. memory.peak 42.2 → 42.6 MB (noise about ±1 MB). S
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
memory.peak over six runs each: 41.3–44.9 → 41.2–43.8 MB; this machine does not show the
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
twice: median wall 272.0 and 270.5 → 255.4 and 254.0 ms (step 6: 297.5 and 295.6). S 849 → 846.
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
245.4 and 244.8 ms. memory.peak over four runs: 42.2–46.2 → 43.7–44.6 MB (the bytes add 0.6 MB
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

Locally, memory.peak over seven runs each: 43.1–45.4 MB (median 44.7) → 41.5–42.5 MB
(median 41.8). The window alone: median 41.9, but one run at 44.9. Cycles and wall unchanged:
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
runs: median 41.7 → 41.8 MB. S 848 → 848. Gate: 507 pass, 0 fail.

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
interleaved runs: median 41.9 → 42.0 MB. S 848 → 852. Gate: 507 pass, 0 fail.

A second board run of the same code, run-7a898680a856 (from the next push), measured 0.2293 s
and 42.2 MB.

## Step 18: 12 bytes of the key in each slot

A profile of step 17 by source line (a build with line tables, `perf record` every 100k user
cycles) put 6.3% of all samples on one line of `TermTable::position`: the comparison of the
token's whole key with `keys[slot.id]`, made when the slot's first 8 bytes match a term longer
than 8 bytes. That load depends on the slot load, so the prefetch made at the token does not
cover it, and `keys` is 2.4 MB. Far from rare: in the first 25 MB of seed 1, 20% of the tokens
are longer than 8 bytes, and 12.7% of all tokens share their first 8 bytes with a long query
term, nearly all of them (98.5%) 9 to 12 bytes long.

A slot now also holds the term's bytes 8 to 11 (`tail`), and the id and the flags share one
u32 (id × 16 + flags), so it is still 16 bytes. `LONG` marks a term past 12 bytes, and only
those read `keys`: 0.2% of the tokens. In the same profile that line falls from 3,763 samples
to 38, and all samples from 59,522 to 56,962. Ids from 2^28 up would not fit; `TermTable::new`
stops with an error before that, at 2^28 term occurrences.

Locally, against step 17: user cycles 5.85–5.89 → 5.68–5.70 G over five rounds of `perf stat
-r 5`, instructions 8.573 → 8.687 G (the extra compare). Alternating runs (`ab.py`), 15 or 16
each, five rounds: median 237.3, 237.9, 237.8, 237.2 and 237.1 → 229.8, 229.8, 230.6, 229.5 and
229.5 ms (−3.2%). memory.peak over six interleaved runs: median 43.2 → 43.0 MB (the table is
the same size). S 852 → 857. Gate: 510 pass, 0 fail.

## Step 19: 99 fewer lines

K weighs S with an exponent of 0.15 against T's 0.5, so at B's numbers one code line is worth
about as much as 0.035% of T. This step only cuts lines, except for one alignment.

- **Pure restructurings.** One `use std::{...}` with `arch::x86_64::*` for the intrinsics
  (clippy's `wildcard_imports` allowed in Cargo.toml) replaces nine `use` lines and the two
  inside `classify` and `prefetch`: the machine code is byte-identical. The libc constants sit
  at their calls, named in comments, except `MADV_DONTNEED`. `fold` builds `ONES` from two
  32-byte arrays. `bool_cover` fits its signature on one line through a `Step` alias.
- **Cold code.** `main` maps both files through one closure and writes the parts with one
  `try_for_each`. `map_file` maps an empty file as one byte past its end instead of a special
  case. `id_at` reports a bad line by its byte offset; an empty `docs.txt` now stops with that
  error, as spec §2.1 has every line end in an LF. The parser looks operators up in a
  three-entry table. `Check::new`, `Check::unless`, the plan's accessors and `Queries::steps`
  each lose a line or two.
- **The term table never doubles.** It is sized once at 3 slots per 2 term occurrences, plus
  one, rounded up to a power of two, so it is at most two thirds full whatever the queries
  hold, and a probe always meets an empty slot. On the seeds that is the same 262,144 slots as
  before (152,904 occurrences, 75,747 distinct terms on seed 1), so the pass is unchanged. The
  rebuild, `place` and `huge_slots` go into `intern` and `new`.
- **The walk.** `pop` clears and returns the lowest set bit, and the last partial block is
  padded once before the loop, so each block is `first_chunk().unwrap_or(&padded)`.
- **The key-mask table on one cache line** (+2 lines). A token's mask is a 32-byte load from
  `ONES` at offset `32 - len`. The table had byte alignment and sat wherever the other
  constants left it: at offset 59 of a cache line at steps 17 and 18, where only tokens of 28
  bytes or more split the load. These cuts moved it, to offset 11 on step 17's code, where
  every token of 10 bytes or fewer split it (split loads 44.3 → 78.4 M, the run 0.7 ms
  slower), and to offset 43 on step 18's (tokens of 12 bytes or more, 42.0 → 43.2 M). Aligned,
  no token splits it, and no later change to the constants can move it.

Locally, against step 18 (alternating runs, 61 each, twice): median 229.8 and 229.7 → 230.1 and
229.9 ms, min 228.3 and 227.9 → 228.8 and 227.8 ms. `perf stat`: user cycles 5,694 → 5,695 M,
instructions 8,687 → 8,739 M (the walk), split loads 42.0 → 42.2 M. The hot code is the same
apart from the walk and `steps`; the rest is code placement, which alone moves a run by up to
0.6% here: step 17 with one no-op `assert!` added to `main` ran 1.35 ms slower than step 17
itself, and with every function aligned to 64 bytes the cold cuts and step 17 ran the same
cycles. memory.peak over 12 alternating runs: median 43.1 → 42.8 MB. S 857 → 758. Gate: 510
pass, 0 fail.

On the board, run-74f2276732d6 measured 0.2241 s, in a run where track A drew 22.9 ms and
track C 38.4 ms, slow draws for both (their usual draws are about 19 and 35 ms).

## Step 20: a larger sample, counted in parallel

The plan picks each query's triggers from how many sampled documents hold each term
([`sample_hits`]). In the first 1,000 documents most query terms have no hits at all, so they
tie, and the plan picks triggers that the pass then checks more often than it needs to. With
nothing else changed but the constant (the same code layout, on step 17's code), the pass's
user cycles on seed 1 were 5.98 G for a 250-document sample, 5.86 G for 500, 5.79 G for 1,000,
5.73 G for 2,000, 5.69 G for 4,000 and 5.66 G for 8,000. Counted on one thread, though, every
1,000 more documents delay the pass by about 5 ms.

The sample is now the corpus's first 1/64 (`SAMPLE_SHARE`; 4,005 documents and 4.0 MB on seed
1), split at line starts into one part per thread. Each thread counts its part
([`count_hits`]), and the parts are summed into the first part's counts, whose pages are
written already: a new zeroed vector would fault each page twice, once for the first load and
again for the first store. The shares 1/128, 1/96, 1/48 and 1/32 came out within noise of 1/64:
a larger sample costs about as much time before the pass as it saves in it.

Locally, against step 19, alternating runs (`ab.py`, 14 each), two rounds: median 229.5 and
229.6 → 226.6 and 226.4 ms; the same change written with a loop instead of `reduce`, in two
other rounds: 229.5 and 230.1 → 226.9 and 227.1 ms. The loop form with every function aligned
to 64 bytes, so that the hot functions, whose code is the same, sit at the same offsets: 230.8
and 230.8 → 226.8 and 226.7 ms. `perf stat`: user cycles 5.59 → 5.51–5.56 G, instructions 8.739 →
8.686 G. In a `perf record` timeline the pass starts at 21.4 ms instead of 21.1–22.0 ms, and
its threads' cycles fall 5.59 → 5.51 G. memory.peak over six interleaved runs: median 41.2 →
41.0 MB. S 758 → 771. Gate: 510 pass, 0 fail.

## Step 21: 35 fewer lines, loops on 64-byte boundaries

Cuts in code that runs once per run or once per thread:

- **One `parallel` helper** (7 lines) runs `work(i)` for each `i` on a thread of its own and
  returns the results in order. The pass, the output and the query parse each spelled out a
  `thread::scope` in 8 to 12 lines; each now takes 2 to 4 (−15 in all). The sample keeps its
  own: it adds each part as its thread is joined, and collecting every part first made the
  sample 0.08–0.19 ms longer (min and median of 15 timed runs).
- **The intern loop without its prefetch 16 terms ahead** (−5). The serial end of the parse
  (table allocation, merge and intern of seed 1's 152,904 term occurrences) took 5.49 ms with
  it and 5.48 ms without, median of 15 timed runs each.
- **The output** adds a query's cells over all chunks and splits the total once (−2): together
  the chunks' id sums stay below `2^SUM_BITS`, as each one does, since `answer_all` bounds the
  sum of all ids. The first matches come straight from the chunks through
  `flat_map(..).take(SHOWN_IDS)` instead of a buffer, and `Chunk`'s fields are `at` and `ids`,
  so its literal fits on one line (−8).
- **`Plan::new`** numbers a stored term's bitset row or id list from one two-entry counter
  (−4), and **`line_bounds`** takes its last bound from the loop: a cut at `docs.len()` finds
  no LF after it (−1).

**Loops on 64-byte boundaries.** Built as before, this step ran slower than step 20 though it
adds no work: alternating runs (`ab.py`, 61 each), two rounds, median 226.4 and 226.2 → 227.2
and 227.0 ms. Each cut alone on step 20, two rounds: the helper +0.41 and +0.38 ms, the intern
loop +0.07 and −0.13 ms, the rest +0.88 and +0.85 ms. The cost is where the hot loops sit. The
`Plan::new` cut alone, which runs once and leaves the instruction count at 8,686.1 M, moved the
scan thread's code, byte for byte the same, from offset 16 of a 64-byte line to offset 0, and
user cycles rose 0.3–0.6%. `-C llvm-args=-align-loops=64` in `.cargo/config.toml` starts every
loop on a 64-byte boundary; executing the padding adds 28 M instructions (8,683 → 8,711 M).
Step 20 built that way ran 226.5 and 226.3 ms, this step 225.9 and 225.8 ms, in the same
sessions as step 20 as built before at 226.0 and 226.0 ms. A loop now keeps its offset in a
line wherever later changes move its function.

Locally, against step 20 (alternating runs, 41 and 61 each): median 226.00 and 225.99 → 225.86
and 225.80 ms, min 224.90 and 225.00 → 224.32 and 224.15 ms. memory.peak over 10 and 16
alternating runs: medians 42.95 and 42.74 → 43.61 and 42.93 MB. S 771 → 736. Gate: 510 pass,
0 fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

## Step 22: 8 fewer lines, the same machine code

Four spots in code that runs once, written so that rustfmt keeps them on fewer lines:

- In `answer_all`, the chain that reads each chunk's first id and the check on the document
  count each put their receiver in parentheses, so each is a one-call chain that fits on one
  line (−5).
- `bit_index_sum` names its per-mask closure, `weight` (−1).
- `TermTable::new` binds the empty `keys` first, so the struct literal fits on one line (−2).

Checks: the built binary's `.text` and `.rodata` are byte for byte those of step 21, and only
88 bytes of `.data.rel.ro` differ (the line numbers in panic locations), so T and memory are
unchanged. S 736 → 728. Gate: 510 pass, 0 fail.

## Step 23: a check's terms in one gather

On seed 1 the per-document checks ([`Check`]) run 16.9 million times. Each one built its truth
table's index from its six terms: per term a bounds-checked load of the document's bit set
`seen`, a shift and an OR. With the loads unchecked, that still took 29 cycles a check:
0.49 G of 5.49 G user cycles on step 20.
Now one AVX2 gather (`vpgatherdd`) loads the 32-bit words of `seen` that hold the six terms'
bits, a variable shift moves each bit to the top of its lane, and `movemask` collects the six
bits as the index. The two spare lanes read word 0 and are masked off. Every term id, and the
pad `miss`, is below `64 * seen.len()`, so no lane reads outside `seen`; the loop that clears
the document's bits at its end skips its bounds check on the same grounds. Gather Data
Sampling, whose mitigation slows gathers, does not affect Raptor Lake (locally the kernel
reports "Not affected").

Locally, against step 21, whose `.text` and `.rodata` step 22 keeps (alternating runs, `ab.py`,
20 each), two rounds: median 226.44 and 225.92 → 215.61 and 215.19 ms, min 224.58 and 224.32 →
214.20 and 213.85 ms. `perf stat`: user cycles 5.64 → 5.30–5.31 G, instructions 8,711 → 7,517
M; branch misses 57.3 → 58.7 M. On step 20, built with every function aligned to 64 bytes, the
unchecked loads alone ran 226.6 → 221.1 ms and the gather on top of them 213.8 ms.
memory.peak over six alternating runs: median 43.5 → 43.2 MB. S 728 → 732. Gate: 510 pass, 0
fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

## Step 24: one loop over a document's checks

A document kept the check range of each BOOL trigger it held, and at its end ran a loop over
each range. On seed 1 the 8.56 million ranges hold 1 check (4.0 M), 2 (2.28 M), 3 (1.17 M), 4
(0.57 M) or more (0.37 M), with no pattern from one to the next, so the exit of nearly every
range's loop mispredicted. Profiled with the last branch record (`perf record -b`, the
mispredicted entries counted by branch address), that exit drew 13% of step 23's
mispredictions, after the term table probe's compare (25%), the stored-term test (22%) and the
first-sighting test (16%).

The trigger now appends its check indices to one list for the document: four written at once
and the list cut back to the range's length, so no branch depends on that length, plus a loop
over the indices past four, which runs for one trigger in 23. At the document's end one loop
runs the whole list.

Locally, against step 23 (alternating runs, `ab.py`, 20 each), three rounds: median 216.91,
217.53 and 215.92 → 212.68, 211.64 and 211.56 ms, min 215.02, 214.46 and 214.59 → 210.38,
210.73 and 210.50 ms. `perf stat`: branch misses 58.5 → 52.0 M, user cycles 5.32 → 5.23 G,
instructions 7,517 → 7,625 M. memory.peak over six alternating runs: median 43.8 → 43.3 MB.
S 732 → 733. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

## Step 25: a token loop without branches on what a token is

Step 24's 52 million mispredicted branches came from four tests per token, counted by branch
address in the last branch record (`perf record -b`): the term table probe's compare (34%),
whether the doc held the term already (20%), whether the term is stored (26%) and whether it
triggers BOOL checks (11%). Each tests a property that follows no pattern from one token to
the next. On seed 1, 79% of the 36.3 million tokens are query terms, 78% of those are new in
their document, and of the new ones 53% are stored and 38% are triggers.

- **Id 0 is no term.** The table's first key is a placeholder, so term ids start at 1, and an
  empty slot, all zero, carries id 0 and no flags. A probe for a token that is no query term
  returns that empty slot, and the pass treats the token as any other: bit 0 of `seen`, id 0
  in the doc's terms, nothing stored or triggered. A token past `MAX_TERM` bytes takes a key of
  0xFF bytes, which no term has. The sample counts its misses at id 0, which no query reads.
- **One branch per probe.** The compare uses `^`, `|` and `&` instead of `&&`, and the test
  that ends the probe (an empty slot, or the term) goes through `black_box`: as one value it
  stays one branch, mostly taken, where LLVM split it into two, the first of which
  mispredicted on every token that is no query term.
- **Lists by store and add.** A conditional move picks the term's flags when the doc did not
  hold it yet, and 0 when it did. The term id is written at the end of the doc's stored-term
  list and at the end of its trigger list, and each list's length grows by the matching flag
  bit, 0 or 1. Both lists have one entry per token, so neither needs a capacity test. After
  the doc's last token, one loop does the stored adds and one appends the triggers' checks, as
  before.

Built up step by step on step 24, median of 14 to 18 alternating runs: the empty slot as the
probe's result for misses, with the compare's two branches as LLVM left them, 211.9 → 209.7 ms; the lists by
store and add on top, 195.0 ms; one branch per probe, 189.6 ms; id 0 for the empty slot
instead of an id written into every empty slot after planning, 187.0 ms; the conditional move
and the `^` compare instead of `&` between booleans, 183.6 ms. Two versions lost. The single
probe branch alone, with the old tests after it, ran 216.8 ms against 209.7: each token's
randomness moved into the first-sighting test. Doing the stored add and the check append for
every token, the spare row and list absorbing the others, cut mispredictions to 24 M but
added 2.8 G instructions: 5.76 G user cycles against 5.18 G.

Locally, against step 24 (alternating runs, `ab.py`, 18 each), two clean rounds: median 212.38
and 211.81 → 183.60 and 183.71 ms, min 210.92 and 210.03 → 182.48 and 183.17 ms. Two other
rounds, run while the box was loaded (load average 7–8), are left out. `perf stat`: branch
misses 52.1 → 20.8 M, instructions 7,625 → 8,392 M, user cycles 5.23 → 4.42–4.44 G.
memory.peak over six alternating runs: median 42.6 → 42.9 MB. S 733 → 738. Gate: 510 pass, 0
fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

## Step 26: stores to clear a document's bits, and the probe's test in a register

- **Clearing by stores.** After a document's last token, a loop cleared each token's bit in
  `seen` with a load, an AND and a store. Tokens whose bits share a word, among them every
  token that is no query term (bit 0 of word 0), made a chain through memory, each load
  waiting on the store before it. Every set bit belongs to one of the document's terms, so
  setting each term's word to 0 clears them all, with a store and no load.
- **The probe's test in a register.** `black_box`, which kept the probe's end test one branch
  in step 25, writes its value to the stack and reads it back, which adds a store-to-load
  forward to every probe. An empty `asm!` block that takes the value in a register hides it
  from LLVM the same way, with no memory access.

Each alone, against step 25 (median of 18 alternating runs): the stores 3.5 ms, the asm block
1.7 ms. Locally, against step 25 (alternating runs, `ab.py`, 18 each), two rounds: median
183.40 and 183.54 → 179.01 and 178.44 ms, min 181.87 and 182.22 → 177.50 and 177.52 ms.
`perf stat`: user cycles 4.39–4.44 → 4.31 G, instructions 8,392 → 8,236 M, branch misses 20.8
→ 20.9 M. memory.peak over six alternating runs: median 42.7 → 43.6 MB. S 738 → 740. Gate:
510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

## Step 27: the queries planned on every thread

On seed 1 the pass started about 21 ms into the run, and one thread spent 4.9 ms of that in
the per-query planning loop: for each of the 50,000 queries, the phrase's trigger term or the
BOOL query's cover, its checks, or its stored terms.

- **The plan in parallel.** Every thread now plans a share of the queries, as the parse does,
  and returns its (trigger term, phrase) and (trigger term, check) pairs and its queries on
  sets, each in query order. `group` reads the parts in order, so each term's phrases and
  checks come out in the same order as before. A term's PHRASE and BOOL flags follow from the
  groups (its group is not empty), and one thread still numbers the stored terms in query
  order, from the queries on sets.
- **No test for empty slots.** The flags were added to the term table slot by slot, behind a
  test for an empty slot, which mispredicts often, as empty slots lie at random. An empty slot
  holds id 0, whose flags are 0, so every slot now takes the add, without the test.

Timeline over three instrumented runs: the loop 4.9 → 1.0–1.1 ms, adding the flags 1.5 → 0.5–0.7
ms, and the pass starts at 16.7–17.2 ms instead of 20.7–21.7. Locally, against step 26
(alternating runs, `ab.py`, 18 each), two rounds: median 178.38 and 178.94 → 174.46 and 174.79
ms, min 177.18 and 177.43 → 173.36 and 173.27 ms. `perf stat`: user cycles 4.30–4.31 → 4.31 G,
instructions 8,236 → 8,254 M. memory.peak over ten alternating runs: median 42.5 → 42.0 MB.
S 740 → 744. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

## Step 28: the sample and the intern without stalls

Three cuts to the work that runs before the pass:

- **The sample probes as the pass does.** Each sampling thread looked a token up as soon as it
  found it, so each probe's slot load stalled in turn, and branched on whether the document
  had counted the term yet. It now collects a line's tokens with a prefetch of each one's home
  slot, probes them at the line end, as [`scan`] does, and adds `counted != doc`, 0 or 1.
- **An intern with no branch on a new term.** Half of the 152,904 term occurrences are new
  terms, at random. The slot is now written either way, with its id or the next one, picked
  by a conditional move, and the key is pushed, then cut off again unless the term is new.
- **One line end for the sample.** The sample's end came from `line_bounds(docs, 64)`, which
  reads around all 64 cuts, faulting in their pages, to use one. `line_end` finds that one.

Timeline over three instrumented runs: the intern 4.7–5.0 → 3.9–4.3 ms, the sample's end 0.3 →
0.05 ms, the sample 4.8–5.1 → 3.4–4.0 ms (a third run of step 27, slowed by other load, took
6.5). Locally, against step 27 (alternating runs, `ab.py`, 20 each), three rounds: median
174.96, 175.52 and 175.31 → 173.39, 173.52 and 173.54 ms, min 174.24, 173.97 and 172.99 →
172.68, 172.50 and 171.83 ms. `perf stat`: user cycles 4.32–4.34 → 4.34–4.36 G, instructions
8,254 → 8,263 M, branch misses 20.8 → 20.5 M. memory.peak over eight alternating runs: median
43.0 → 42.4 MB. S 744 → 746. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their
reference hashes.

One probe helper shared by the sample and the pass, a line shorter, changed how the pass's
loop was compiled and ran 183.8 ms against 175.0 for step 27, so the pass keeps its closure.

## Step 29: the sets path

On seed 1, 1,773 queries run on sets, over 350 bitset rows and 2,787 id lists. Each thread
flushes a window every 4,096 documents, and the pass adds 9.9 million row bits and 1.9 million
list entries.

- **One sum per query and window.** The flush handed each of a query's 64 result words to
  `add_word`, which loaded and stored the query's cell each time, so the 64 calls formed a
  chain through memory. `add_bits` now records the first matches one by one while the query
  lacks them, as before, and then counts the whole window in one sum over its words, which the
  compiler vectorizes: per word a popcount, and the id sum as before ([`bit_index_sum`]).
- **A `u32` store code.** A term's [`Store`] was an 8-byte enum; it is now a `u32`, `k` for
  bitset row `k` and `LIST | j` for id list `j`, which halves the per-term array the pass reads
  for every kept term, and takes 6 lines fewer.
- **Rows and lists listed apart.** The branch on whether a kept term is a row or a list drew a
  fifth of the remaining mispredictions (last branch record, by address), as the two kinds
  follow no pattern. A slot now carries two flags, ROW and LISTED, in place of STORED, and the
  token loop lists each kind apart by a store and an add, so each loop of adds has one kind.
  The term id moves up a bit, so the table takes up to 2^27 term occurrences.

Built up on step 28 (median of 18 to 20 alternating runs, rounds against the same step 28
binary): the `u32` code 173.74 → 172.14 ms; the sum per window on top 172.05 → 168.78; the
lists apart on top of that 169.72 → 168.34. Locally, against step 28 (alternating runs,
`ab.py`, 18 to 20 each, with other jobs on the box, load average 4–7), three rounds: median
173.14, 174.52 and 174.12 → 168.32, 168.92 and 169.58 ms, min 171.99, 172.55 and 172.64 →
167.31, 167.11 and 167.60 ms. `perf stat`: user cycles 4.30–4.31 → 4.15 G, instructions 8,263
→ 8,261 M (the sum saves about 310 M, the second list adds as many), branch misses 20.5 →
18.3–18.4 M. memory.peak over eight alternating runs: median 41.3 → 40.8 MB. S 746 → 745.
Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

Comparing a phrase's span term by term instead of as slices, which calls `bcmp`, gained
nothing: median 171.91 ms against 171.52 ms, 18 alternating runs each.

A second run of the step 29 code, run-2d06517dbcd4, measured 160.2 ms and 42.2 MB (73.8 points).

## Step 30: a plain static executable

`.cargo/config.toml` adds `-C target-feature=+crt-static` and `-C relocation-model=static`, so the
binary is a plain static executable at a fixed address, with no dynamic loader and no
position-independent code. In track A the same change ran 3.9% fewer cycles, with half the iTLB
misses; here the gain is smaller. A static PIE alone measured nothing at step 14 (experiments
below).

Local, turbo off: `ab.py` against step 29, 16 interleaved runs, five rounds: medians 168.49 →
168.05, 169.05 → 168.17, 169.66 → 168.04, 168.57 → 167.54 and 168.25 → 168.58 ms, minimums
167.37 → 165.93, 167.15 → 166.76, 167.45 → 166.86, 167.84 → 166.44 and 167.50 → 166.34 ms.
S unchanged at 745. Gate: 510 pass, 0 fail.

## Step 31: the token loop's stores without bounds checks

After step 29 the token loop ran about 100 instructions per token. It is a closure, and the
four per-token vectors it wrote (the term ids, and the rows, lists and BOOL triggers kept) were
captured `Vec`s: LLVM cannot tell that a store into one of their buffers leaves the `Vec`
headers alone, so after every store it reloaded each header's pointer and length from memory,
and checked each index against the length. With the loop's registers taken, more spilled.

- **One slice, four runs.** The four vectors are one now, sliced to `4 * n` entries for a
  document of `n` tokens: the term ids from 0, then the rows, lists and BOOL triggers kept from
  `n`, `2n` and `3n`, each up to its cursor. The slice's pointer stays in a register, and the
  writes take no bounds check: a cursor moves at most once per token, so it stays in its run.
- **The seen word unchecked.** A token's word of `seen` is read and written without a bounds
  check, as the clearing loop already did (step 23); an `assert!` where `seen` is sized states
  the bound.
- **The probes read in place.** The loop reads each (key, home slot) from the probe list
  instead of moving it out with `drain`, which copied the 32-byte key to the stack.

Built up on step 29 (local, 18 to 20 alternating runs, rounds against the same step 29
binary): the three kept vectors unchecked 168.95 → 165.97 ms; with the seen word and a fourth
vector for the term ids, 169.08 → 163.80; the four as one slice 169.08 → 158.80 (the same
round); the probes read in place on top of that, 160.05 → 158.60 and 159.58 → 157.92 ms.
Locally, against step 30 (`ab.py`, 20 alternating runs each, with other jobs on the box, load
average 7–11), three rounds: median 169.99, 172.52 and 169.26 → 158.49, 159.94 and 158.42 ms,
min 167.46, 166.78 and 167.99 → 156.55, 157.30 and 156.29 ms. `perf stat`: user cycles 4.01 →
3.62 G, instructions 8,268 → 7,338 M, branch misses 18.2 → 18.1 M. memory.peak over eight
alternating runs: median 41.3 → 40.9 MB. S 745 → 746. Gate: 510 pass, 0 fail; seeds 2, 3, 7,
10 and 12 match their reference hashes.

Taking `seen` as a slice inside the closure cut 81 M more instructions but did not measure. A
flag in the term table marking a home slot that a term was displaced from, so that a probe
ends at its home slot unless the flag is set, sent 1.39 million of the 36.8 million lookups
past the home slot and cut branch misses from 18.4 to 15.6 M, but it added 91 M instructions,
the user cycles stayed at 4.13–4.16 G, and the wall did not move: 169.69 → 169.10 and 168.87
→ 169.29 ms, for 9 more lines. Without `#[inline(always)]`, its lookup was no longer inlined
and the run took 11 ms longer.

## Step 32: the sources mounted, not copied

The Dockerfile's build stage mounts its context (`RUN --mount=type=bind,target=/src`) instead of
copying it with `COPY . .`, and cargo builds into `--target-dir /out`, since the mount is
read-only. `harness/dockerfile_check.py` accepts a bind mount of the context, and the board
built track A's Dockerfile with the same mount (its step 45). A local `docker build
--network=none` gives a binary byte-identical to step 31's, so T and M cannot move. S 746 →
745.

## Step 33: the first matches without a branch on a check's result

After step 31, `Tally::add_check` still took a branch on each check's result: step 11 made the
add to the cell a conditional move, but recording a first match (`hit`, the check's count at most
`SHOWN_IDS`, and its query's count below `SHOWN_IDS`) branched on `hit` first. A check's result
follows no pattern and comes at the end of a chain of loads, the check's terms and then the
gather of their words of `seen`, so the branch resolved late and often missed.

[`Tally::add_checks`] now takes all of a document's triggered checks. For each, it adds to the
cell with a conditional move as before, writes the check's query at a cursor over the front of
the list of checks, and moves the cursor on when the check matched and is within its first
`SHOWN_IDS` matches, so no branch depends on the result. A second loop then records the first
matches from the queries before the cursor: 678 K of the 16.9 M checks evaluated reach it, and
502 K are recorded. It is a method so that LLVM knows the stores to the cells and to the list
leave `self` alone, and keeps the addresses of its vectors in registers.

Local, against step 31 (`ab.py`, 20 alternating runs, load average 2–6), four rounds: medians
156.66, 157.64, 157.21 and 156.48 → 152.23, 152.33, 153.10 and 152.11 ms, minimums 155.37,
155.21, 155.82 and 155.02 → 151.04, 151.06, 150.88 and 150.80 ms. `perf stat`: user cycles 3.82
→ 3.69 G, instructions 7,338 → 7,257 M, branch misses 18.1 → 16.9 M. memory.peak over five
alternating runs: median 41.3 → 41.3 MB. S 745 → 750. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10
and 12 match their reference hashes.

Moving the loop into the method alone, with the branch on `hit` kept, cut instructions to
7,202–7,270 M but left branch misses at 18.1 M and user cycles at 3.79–3.82 G, inlined or not.
Writing every check's (query, document) into the spare capacity of the first-match list and
adding to its length only for a first match cut branch misses to 16.3 M but added 121 M
instructions, for 3.81 G cycles. A single compare of the cell against a limit that is 0 on a
miss became two branches again, with 22.9 M misses and 4.03 G cycles; held in a register by an
empty `asm!` block, it stayed one compare, but the misses stayed at 18.1 M.

## Step 34: the term lookups without bounds checks

After step 33 each token's lookup still checked bounds: [`TermTable::position`] checked each
slot it probed, `find_from` checked the index that `position` returned, and `prefetch` took a
reference, which cost a check to make, for each token's home slot and each BOOL trigger's
checks and cells. The token loop also reached the term table and `seen` through the closure's
captured references, so it reloaded their addresses after its stores.

- **The probe unchecked.** `position` and `find_from` are `unsafe fn`s that take a slot index,
  as [`TermTable::home`] gives one: it shifts the hash down to the slot count's bits, and the
  probe wraps at the count. The table has 2 slots or more, so that the shift stays below 64.
- **Prefetches by pointer.** `prefetch` takes a raw pointer, made with `wrapping_add`: a
  prefetch of any address is harmless.
- **The table and `seen` as locals.** The token loop takes `plan.terms` and a slice of `seen`
  once per document. Step 31 found the slice alone did not measure, for 81 M fewer instructions;
  with the table, it cut 99 M.

Built up on step 33 (`perf stat`): the prefetches by pointer, instructions 7,257 → 7,220 M; the
probe unchecked as well, 7,079 M, user cycles 3.69 → 3.66 G; the table and `seen` as locals,
6,980 M and 3.56–3.64 G. Branch misses stayed at 16.8–16.9 M. `ab.py`, 18 alternating runs, two
rounds, step 33 / the probe unchecked / all three: medians 152.84 / 151.19 / 150.65 and 152.63 /
150.84 / 150.26 ms, minimums 150.77 / 149.93 / 149.18 and 151.52 / 149.94 / 149.35 ms.

Local, against step 33 (`ab.py`, 20 alternating runs, load average 3–5), three rounds: medians
153.42, 153.74 and 152.45 → 151.11, 151.83 and 151.14 ms, minimums 151.91, 152.08 and 151.64 →
149.87, 148.39 and 149.27 ms. `perf stat`: user cycles 3.69 → 3.56–3.65 G, instructions 7,257 →
6,980 M, branch misses 16.9 → 16.8 M. memory.peak over five alternating runs: median 41.9 → 41.6
MB. S 750 → 751. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their reference hashes.

Splitting the token loop in two, all of a document's lookups first and then the rest from their
flags, added 185 M instructions and 0.4 M branch misses, for 3.65 G cycles against 3.64 G. A
term table of 4-slot buckets, one 64-byte line each, with a key's home slot at the start of a
bucket and the key compared with all four slots in two AVX2 compares, kept the linear probe only
for long keys and for misses in a bucket whose last slot is taken, 0.9% of lookups: branch
misses 16.9 → 12.4 M, but instructions 7,257 → 7,697 M and user cycles 3.70 → 3.85 G.

The board's run of this step, run-401b90111691, drew 161.8 ms, against 145.5 ms for step 33's:
a slow draw. In the same run, track C's unchanged code drew 48.4 ms, against its usual 29 to
31 ms.

## Step 35: fewer instructions in the corpus pass

After step 34 the pass ran 6,980 M instructions on seed 1, about 189 per token, at close to the
core's limit of instructions per cycle, with no single hot spot: the probe took about 16% of the
cycles, the key and its hash 10%, the checks 9%, the BOOL triggers 6%, and the rest was spread
over the tokenizer, the seen bits, the rows and lists, and the phrases. So this step cuts
instructions wherever the machine code showed reloads, bounds checks or tests it did not need.

- **The checks unchecked.** [`Tally::add_checks`] is an `unsafe fn` whose indices are a
  document's checks, so it reads each check and its cell, and writes the first-match queries,
  without the three bounds checks that took 6 of its 33 instructions per check.
- **The triggered checks in a slice sized once.** Each term triggers once per document and no
  two terms share a check, so a document has at most all the checks: their indices go into a
  slice of that length plus 4, allocated once per thread, at a cursor, instead of a `Vec` that
  tested its capacity and was truncated once per trigger. The term's range of checks is read
  from a local slice of `check_at`, whose address stays in a register; so are the slices of the
  stores and rows that the rows and lists loops index.
- **The carried token picked once per block.** A token still open at a block's end closes at
  the next block's first end. [`walk`] picked its start with a conditional move at every end;
  it now picks the start that the block's first end closes once per block, by a conditional
  move, and each end takes the next start with no test.
- **A key in fewer steps.** `key_in` takes the token's 32-byte window with one bounds check
  instead of two, and `key_hash` folds two words instead of four. A key of 16 bytes or less
  hashes as before, since its last two words are zero, so their slots do not move.
- **Phrases compared inline.** A phrase's terms are compared with the document's in a loop
  that ORs their differences, instead of a call to `memcmp` per candidate, which also reloaded
  three registers after each call.
- **The long-key test in the tail.** No letter or digit has the top bit of a byte, so a slot's
  `tail` keeps in its top bit whether the term runs past 12 bytes, and the probe's compare of
  `head` and `tail` covers it: the `LONG` flag and its test at each probe step are gone.

Measured on battery with the power-saver profile (EPP `power`), so no wall times: `perf stat`
counts only, battery, not comparable with the steps before. Built up on step 34, instructions:
the long-key mark in the tail 6,980 → 6,962 M; `add_checks` unchecked 6,823 M; the local slices
for the rows and lists 6,826 M, with user cycles 24–54 M lower in two rounds; phrases inline
6,751 M, with branch misses 17.0 → 17.6 M; the checks in a sized slice 6,556 M, branch misses
16.6 M; the window with one bounds check 6,444 M; the two-word hash 6,370 M; the carried token
picked once per block 6,179 M.

Against step 34 (`perf stat -r 3`, three alternating rounds; battery, not comparable): user
cycles 3,661–3,672 → 3,361–3,391 M, instructions 6,980 → 6,178 M, branch misses 16.9 → 16.6 M.
memory.peak over five alternating runs: median 42.2 → 41.5 MB. S 751 → 761. Gate: 510 pass, 0
fail; seeds 2, 3, 7, 10 and 12 match their reference hashes. The board's run,
run-8bf61f0fec95, measured 131.3 ms and 43.3 MB, against 145.5 ms and 41.3 MB for step 33's.
On AC power again (profile `performance`, turbo off), `ab.py`, 20 alternating runs, two rounds:
medians 151.75 and 150.43 → 140.81 and 140.10 ms, minimums 149.67 and 148.58 → 139.43 and
139.21 ms, 6.9 to 7.2% faster.

Peeling the carried token out of the loop behind a branch cut instructions to 6,259 M but added
0.6 M branch misses, as whether a block starts inside a token follows no pattern; peeling only
the block's first end, with the conditional move kept there, cut them to 6,332 M. Writing the
rest of a range of more than four checks through a `zip` added 120 M instructions; writing it
at an offset from the cursor, one line shorter, added 8.5 M and 0.1 M branch misses.

## Step 36: a shorter serial start

After step 35 the pass started 14.2–14.9 ms into a run on seed 1 (local, on AC, turbo off, a
build with timestamps, 12 threads on the P-cores, medians over three rounds of 11 or more
runs). Interning the query terms on one thread took 3.7 ms, the sample 3.4–3.7 ms and the
planning 1.3 ms, all of it before the pass.

- **The intern prefetch put back.** [`TermTable::intern`] takes the parsed keys and an index,
  and starts loading the home slot of the key 16 places on, so that its probe seldom waits:
  interning 3.60 → 3.02 ms. After step 33 it saved about 0.35 ms for 3 more lines and was
  left out; it now takes 2.
- **Lengths instead of keys.** The table kept each term's 32-byte key, pushed at each occurrence
  and dropped again unless the term was new: 2.4 MB for seed 1's 75,747 terms. The costs read
  only the keys' lengths, and the probe only the keys longer than 12 bytes, which a slot does
  not hold whole (13 terms on seed 1). The table now keeps each term's length in a byte, and the
  long keys in an array of zeroed entries, one per term occurrence and one for id 0: only a new
  long term writes its key at its id, and every other key is written to entry 0, which no probe
  reads, so that no branch is taken. On seed 1 its written entries take one 2 MiB huge page.
  Interning without the prefetch: 3.73 → 3.60 ms.
- **Costs where the planning reads them.** A term's cost is computed from its sample hits and
  its length when a query is planned, instead of read from a table that one pass over the hits
  and the keys filled first: the planning 1.30–1.34 → 0.98–1.06 ms.
- **The sample in one cell per term.** [`count_hits`] kept, per term, a count and the last
  document counted in two arrays, so each token cost two random accesses, and the second
  array, 606 KB per thread, took 148 page faults on each. A `u32` cell now holds the count in
  its high half and the last document's number in its low half: the sample 3.40–3.72 →
  2.33–2.46 ms.
- **The flags in bytes.** The flags that [`TermTable::mark`] adds to the slots are collected
  in bytes instead of `u32`s: 0.15–0.16 → 0.04 ms.

In the round that timed both, the pass started at 14.24 ms for step 35 and 12.06 ms for this
step; over three rounds each, at 14.24–14.86 and 12.06–12.50 ms, and the pass itself took
118.8–121.6 and 119.5–119.8 ms.

Local, against step 35 (`ab.py`, 20 alternating runs on the P-cores, on AC), two rounds: medians
139.93 and 140.59 → 137.99 and 138.03 ms, minimums 138.94 and 138.95 → 135.71 and 136.54 ms.
`perf stat` on CPUs 12-15: instructions 6,080 → 6,090 M, page faults 11,949 → 10,560; on the
P-cores, page faults 19,235 → 16,669. memory.peak over 12 alternating runs on the P-cores:
median 42.3 → 41.4 MB. S 761 → 761. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their
reference hashes.

The grouping's arrays written when allocated, as tried after step 17, took the checks' grouping
from 1.27 to 1.32 ms, with 10,564 page faults against 10,560. The prefetch 8 or 32 places on:
interning 3.17 and 3.15 ms, against 3.09 at 16. The table's flags added on every thread, a share
of the slots each: marking and the rest of the plan 0.83 → 0.69 ms, for 3 more lines. On
battery, the plan's groups placed on every thread, a range of keys each, took the phrases'
grouping from 0.47 to 1.33 ms and the checks' from 2.03 to 1.65 ms, and the start from 20.51 to
21.53 ms; the sample's counts summed on every thread after a barrier took the start from 19.13
to 18.92 ms, for 15 more lines. A long key found by a binary search in a sorted list of the long
terms, or in a cold function, instead of in an array by id, added 200–302 M instructions to the
pass, as the probe loop then spilled registers.

The board's run, run-ce4c2e1d13c6, drew 134.2 ms against 131.3 ms for step 35's, on a slow box:
in the same run, track C's unchanged code drew 60.1 ms, against its usual 27 ms.
A second run of the same commit, run-fa751616d535, drew 169.1 ms and 39.9 MB, while track C's
unchanged code drew its usual 27.2 ms in it, so this draw was slow for B alone.
A third, run-ab34f37502a1, drew 131.2 ms and 42.6 MB.

## Step 37: the term table's huge pages asked for again

On the board, B's wall time follows its memory.peak. Step 36 drew 134.2 ms at 42.8 MB, 169.1 ms
at 39.9 MB and 131.2 ms at 42.6 MB, and step 34 drew 161.8 ms at 40.5 MB, the lowest memory of
steps 30 to 36. A run whose blocks get no transparent huge pages takes less memory, as nothing
is rounded up to 2 MiB, and its random reads into the term table walk the page tables.

Locally, step 36 run with THP off for the process (`prctl(PR_SET_THP_DISABLE)`, which `execve`
keeps), on the P-cores, medians of four runs with and four without: cycles 3,514 → 3,726 M
(+6.0%), instructions 6,336 → 6,339 M, dTLB load misses 2.5 → 22.3 M, page faults 16,644 →
17,664, memory.peak 41.5 → 39.1 MB. Sampled without THP (`mem_inst_retired.stlb_miss_loads`),
58% of the loads that miss the STLB are the prefetches of each token's home slot, and 22% the
BOOL triggers' prefetches of their checks and cells. With THP, 2.0 M remain, 45% of them at the
checks' prefetch: the checks take 1.6 MB, too little for a huge page.

- **The collapse.** Once the table is written, `madvise(MADV_COLLAPSE)` (Linux 6.1 on) collapses
  its 4 KiB pages into huge pages if its faults found no free one, synchronously, with direct
  compaction if needed, whatever the sysfs THP settings say (`madvise(2)`). Where the faults got
  huge pages, the call returned in 33–172 µs in three runs. With THP off while the table is
  written and back on before the call, the collapse took 1.38–1.48 ms and gave back the counts
  with THP: cycles 3,521 M, dTLB load misses 2.50 M, memory.peak 41.1 MB. With THP off for the
  whole process, it fails and changes nothing: cycles 3,736 M.

In 12 alternating runs each, neither step 36 nor this step lost the table's huge pages on this
machine, although 5% of its THP faults since boot fell back to 4 KiB pages and 91% of its direct
compactions failed (`/proc/vmstat`). So the collapse helps where the faults could not, or did
not try to, compact memory, and costs a fraction of a millisecond elsewhere.

Local, against step 36 (`ab.py`, 20 alternating runs on the P-cores, on AC, load average 4–6),
six rounds: medians 139.16, 137.95, 137.61, 138.70, 138.55 and 137.73 → 138.67, 138.71, 138.53,
138.27, 138.88 and 137.75 ms, minimums 136.68, 136.40, 136.31, 136.48, 137.41 and 136.07 →
136.86, 136.79, 137.34, 136.68, 137.12 and 136.98 ms: no change beyond the noise. memory.peak
over 8 alternating runs: median 42.2 → 41.1 MB. S 761 → 762. Gate: 510 pass, 0 fail; seeds 2, 3,
7, 10 and 12 match their reference hashes.

A table of half the slots, 131,072, at most 58% full, took the dTLB load misses without THP from
22.3 to 13.7 M, but its longer probes took 4.3% more instructions: cycles 3,726 → 4,175 M
without THP and 3,514 → 3,983 M with it. Prefaulting the table with `MADV_POPULATE_WRITE`
instead of the stores goes through the same faults, so it gets huge pages no more often. Not
built: a second, small table for the commonest terms, probed first. In the first 16 MB of seed
1, the 4,096 commonest query terms are 64% of the tokens, so those probes would stay on a few
dozen pages, but every other token would probe both tables.

## Step 38: no frees after the output, and its numbers without `write!`

After the pass, a run of step 36 spent 3.6 ms more (local, on AC, a build with timestamps, the
P-cores, medians of 8 runs): formatting took 1.83 ms on every thread, the frees when
`answer_all` returned 1.14 ms, and writing the file 0.63 ms. Step 37 changes nothing there.

- **No frees after formatting.** The plan and the chunks are `ManuallyDrop`, so `answer_all` no
  longer unmaps their blocks before it returns: the process exits once the output is written
  ([`main`]), which already left the output itself unfreed. The frees: 1.14 → 0.06 ms, for the
  queries, still dropped. After step 33 the same leak, plan, chunks and queries, measured −0.96,
  +0.10 and −1.80 ms in three rounds of 20 runs, too noisy to keep it.

- **Numbers by `format_into`.** `format_into`, with a `NumBuffer`, writes a number's digits
  without the formatting machinery of `write!`. Each number goes out followed by a space, and
  the line's last space becomes its LF: formatting 1.83 → 1.44 ms.

In the timing build, a run took 135.65 → 134.14 ms (median of 8).

Local, against step 37 (`ab.py`, 20 alternating runs on the P-cores, on AC, load average 4–6),
four rounds: medians 138.53, 138.27, 138.88 and 137.75 → 137.86, 138.38, 139.08 and 137.15 ms,
minimums 137.34, 136.68, 137.12 and 136.98 → 135.85, 136.61, 137.15 and 136.31 ms. memory.peak
over 8 alternating runs: median 41.1 → 41.1 MB. S 762 → 763. Gate: 510 pass, 0 fail; seeds 2, 3,
7, 10 and 12 match their reference hashes.

The board measured steps 37 and 38 together, in run-5de0cb146d0e, on a slow box: 144.5 ms at
41.4 MB, while track C's unchanged code drew 45.9 ms in the same run, against 25.2 ms in the one
before, and track D's 147.7 ms, against 122 to 132 ms.
A second run of the same code, run-0ea6dd17c4d2 (with track C's step 36), drew 129.1 ms at
39.6 MB.

## Step 39: the checks on huge pages, and the sample summed as it comes in

After step 37, 2.0 M dTLB load misses remained, 45% of them at the BOOL triggers' prefetch of
their checks: 40,710 checks on seed 1, 1.63 MB, too little for a huge page of their own.

- **The checks on huge pages.** One helper, `huge`, now allocates both the term table's slots
  and the grouped checks: alone, at a 2 MiB boundary, never freed, `MADV_HUGEPAGE` before the
  writes and `MADV_COLLAPSE` after them, as in step 37. Its size is rounded up to whole 2 MiB
  pages, as a huge page maps only a whole aligned 2 MiB of a block: a block of 1.63 MB got none.
  The counting sort writes the checks straight into it (`group_into`). `perf stat -r 3`, three
  alternating rounds: dTLB load misses 2.49–2.59 → 0.41–0.47 M, dTLB store misses 0.28–0.30 →
  0.21 M, page faults 16,646–16,674 → 16,237–16,268, cycles 3,401–3,502 → 3,444–3,455 M,
  instructions 6,273 → 6,271 M.

- **The sample summed as it comes in.** Each thread adds its part of the sample's counts to the
  sum under a lock as soon as it has counted, and frees it. Before, the main thread joined all
  the parts first, then summed them and freed them one by one, 0.56 ms after the last part was
  counted; now the sum is ready 0.03 ms after it (a build with timestamps, medians of 10 runs).

- **The counting sort from the back.** `group_into` counts into an array written when
  allocated, so the counts' loads find no shared zero page (step 16), and puts the items in
  last first, each key's end moving back to its start, so the starts need no rotation.

In the timing build (medians of 10 runs), the walks begin at 14.78 ms instead of 15.36, and the
passes end at 131.81 ms instead of 133.87.

Local, against step 38 (`ab.py`, 20 alternating runs on the P-cores, on AC, load average 2–6),
six rounds: medians 137.54, 137.87, 136.85, 137.32, 137.11 and 137.23 → 134.96, 135.30, 136.33,
135.57, 135.05 and 135.66 ms, minimums 135.87, 136.16, 135.55, 136.13, 136.14 and 136.46 →
133.38, 134.02, 134.19, 134.09, 133.95 and 134.47 ms. memory.peak over 28 alternating runs:
median 41.45 → 41.59 MB, highest 42.84 → 43.72 MB, as the checks' block takes 2.10 MB where
they took 1.63. S 763 → 769. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their
reference hashes.

## Experiments that did not stay

- **After step 38, on the serial start** (local, a build with timestamps, seed 1, medians of 10
  runs, in ms since the process was spawned; `main` starts at 2.0, as a trivial static binary's
  does). The main thread alone spawns the parse threads (2.16–2.59), allocates the term table
  (0.94), interns the terms (3.02, 126 user instructions per occurrence at 4.3 per cycle), sums
  the sample (0.59), groups the phrases (0.26) and the checks (1.34), sets the stores (0.28),
  marks the table (0.73) and frees the planning's lists (0.18): about 8.5 ms, against about 4.5
  ms in which every thread works. Each scanning thread then takes 0.8–1.0 ms to set up before
  its walk begins at 15.2–15.7 ms. None of these changes paid:
  - The intern in a function of its own: 3.02 → 3.35 ms, and the pass about 4 ms slower in the
    same build, from the moved code.
  - The terms' keys touched before the intern, or prefetched 48 keys ahead: 3.05 ms.
  - The term table zeroed by the kernel, `mmap` and `MADV_POPULATE_WRITE`: 0.85 → 0.86 ms.
  - The table marked through the slot positions the intern recorded: 0.61 → 1.37 ms.
  - The corpus faulted in ahead of the pass. With `MADV_POPULATE_READ` on the main thread:
    9.1 ms, for a pass about 2 ms shorter. From helper threads, the intern waited about 10 ms on
    the `mmap_lock`; per chunk in the scanning threads, 9–10 ms, serialized on the same lock.
    Helper threads reading a byte per page at nice 19: the parse, sample and plan about 2 ms
    slower, the pass about 1.6 ms faster, the wall 138.41 → 138.00 ms.

- **After step 33, on the serial parts** (local, a build with timestamps, seed 1). The pass
  starts at 15.4 ms: parsing the queries takes 7.7 ms, of which interning 3.4, the sample 3.5,
  planning the queries 2.9 and marking the term table 1.1 ms. The threads' passes run 129.5 to
  131.6 ms. At its end, a thread spends 0.3–1.1 ms dropping its pages of the corpus, 0.5–0.75
  ms on its last window flush, 0.3–1.25 ms freeing its sets, 0.2–0.9 ms merging its cells and
  1.3–2.3 ms grouping its first matches. Formatting takes about 1.9 ms, the frees after it 1.3
  ms and the write 0.5 ms. None of these changes paid:
  - The intern loop's prefetch 16 terms ahead (dropped at step 21) put back, or 8 or 32 ahead:
    interning took about 0.35 ms less, for 3 more lines.
  - The queries, plan and chunks leaked instead of freed after formatting, as after step 17:
    three rounds of 20 runs, medians −0.96, +0.10 and −1.80 ms.
  - Nothing freed at the end of a thread's pass: median 152.28 → 152.27 ms, memory.peak 41.4 →
    50.8 MB.
- **PGO** (local, turbo off, on step 31's code): a profile from three runs on each of full seeds
  2–4, passed as `-C profile-use`, with every function matching it. `ab.py`, 16 interleaved runs,
  medians / minimums: 161.21 / 157.44 → 168.09 / 160.76 ms and 158.09 / 155.53 → 161.43 / 159.20 ms
  on seed 1; 158.11 / 156.41 → 161.79 / 159.80 ms and 163.53 / 156.80 → 168.75 / 161.82 ms on seed
  10, which it was not trained on. `perf stat -r 5`, twice: instructions:u 7.34 → 7.73 G, cycles:u
  3.83–3.87 → 3.84–3.93 G, branch-misses:u 18.1 → 17.8 M.
- **After step 17, on the serial start and the end** (local, alternating runs, 16 each, against
  step 17 at 238.5–239.6 ms median). Where the time goes on seed 1: 2.1 ms from spawn to `main`,
  parsing to 8.5 ms, the sample 5.3 ms, the per-query planning loop 5.2 ms and the rest of the
  plan 2.9 ms, so the pass starts at about 22 ms. The threads' passes end over a 4–9 ms spread.
  After its pass, a thread spends 0.6–0.8 ms on its last window flush, 0.1–0.3 ms each on
  freeing the window and merging the check cells, and 1–2.7 ms grouping its first matches.
  Formatting takes 1.5 ms, joining the workers and freeing the chunks, plan and queries 1.3–1.4
  ms, writing 0.6 ms, and the exit 0.8 ms. None of these changes paid:
  - The sample counted by all threads, one part each, then summed: the plan was ready 1.3 ms
    sooner (21.4–21.8 → 20.0–20.7 ms), but the wall did not move (239.6 → 240.1 and 238.5 →
    239.1 ms), for 16 more lines (kept at step 20). The sample's slots prefetched as in the pass
    instead (4 more lines) cut the sample from 5.3 to about 4.1 ms: 239.0 → 239.6 and 239.0 →
    240.3 ms (kept at step 28).
  - The queries, plan and chunks leaked instead of freed after formatting: the exit then took
    1.7–1.9 ms instead of 0.7–1.0, and the wall went from 238.5 to 240.6 ms.
  - The grouping's two arrays written when allocated, as in step 16: 238.9 → 239.5 ms. Each
    thread's first-match list reserved at its bound (`SHOWN_IDS` per query), so it never moves
    while it grows: 238.5 → 238.6 ms.
- **A static-pie build** (`+crt-static`, as tracks A and C use), on step 14 (local, turbo off):
  `ab.py`, 16 interleaved runs, median / min 246.00 / 245.60 ms against 245.75 / 244.05 ms. The
  dynamic loader's quarter of a millisecond is lost in a 245 ms run.
- **Transparent huge pages off on top of the fixed threshold** (`prctl(PR_SET_THP_DISABLE)`):
  41.1–41.2 MB against 42.4–45.3 in the same session, but cycles 9.79–9.83 → 9.91–9.97 G and
  elapsed 0.383–0.385 → 0.392–0.397 s. Huge pages off without the fixed threshold gave
  49.8–50.8 MB, and `malloc_trim(0)` after planning 49.2–50.1 MB.
- **Every BOOL query checked per document, with no sets at all** (step 3 without the sets):
  1.29 s, 29.0 G cycles, 53.3 MB, against 0.67–0.75 s and 14.0–14.2 G for step 2 in the same
  session. That meant about 100 million evaluations, each chasing a query's heap-allocated
  terms and operators.
- **Step 3a's two paths with per-term arrays instead of table slots:** 15.1 G cycles, 0.64 s,
  76 MB, against 13.2 G and 0.58 s for step 2. With 75,747 terms, every hit touched about six
  random cache lines: the 1 MB slot array, 2.4 MB of keys, the last-seen array, the store array,
  and two 1.8 MB arrays of `Vec` headers. Moving the flags into the slots fixed that.
- **Term table grown at 3/4 load instead of 1/2** (2.1 MB of slots instead of 4.2): cycles 11.07 →
  11.53 G, elapsed 0.535 → 0.55 s, memory 81 → 77.7 MB. The longer probe runs cost more than the
  smaller table saved.
- **One per-document loop shared by the sample and the pass**, taking closures: fewer code
  lines, but cycles 10.0 → 10.6 G and instructions 12.25 → 13.60 G. A shared probe function
  that returned the slot cost 4% more cycles on its own. The pass keeps its loop inline.
- **A term table sized for every term occurrence up front** instead of doubling: parse
  13.8–14.7 → 10.7–11.0 ms, but 8.4 MB of slots instead of 4.2, as the queries hold 152,904 term
  occurrences and only 75,747 distinct terms.
- **Huge pages without the `realloc` path**, every resize a new block and a copy: median wall
  288.9 ms against 295.5 for step 6 (285.3 with it). The parse and the plan grow their arrays
  by copying. With `alloc_zeroed` also sent to glibc's `calloc` as well: 288.4 ms.
- **Huge pages only for blocks of 4 MiB or more** (the term table and its keys): the same
  cycles and memory as 2 MiB.
- **No branch per token at the line end, on top of step 6** (kept at step 25). A token that
  is no query term resolved to an empty slot with a reserved id and no flags, and the trigger
  lists grew by writing each entry and advancing by 0 or 1. Branch misses fell from 77.6 to
  65.1 M, but instructions rose from 10.07 to 10.57 G and cycles stayed at 7.19–7.20 G.
  Prefetching the triggered checks after the token loop, or at first sight behind one branch,
  was 0.5–1% slower than step 6.
- **The full keys of long tokens prefetched** in a pass between hashing and probing, on a
  variant of step 5 that was never merged: cycles 8.40–8.44 → 9.09–9.12 G, as the extra pass
  stalls on the slot loads it reads. On step 6, skipping the long-key comparison altogether
  (wrong answers, as an upper bound) saved only 1.5–2.8% of cycles, too little for a prefetch
  scheme to chase.
- **Phrase triggers pushed without a branch** (each position pushed, then dropped again unless
  the term triggers phrases), on top of step 9: branch misses 68.8 → 67.2 M, but instructions
  10.06 → 10.44 G and cycles 6.80 → 6.87 G.
- **Every BOOL query on sets, a window at a time, on step 25** (a prototype). Common terms keep
  their bitset rows; for the others the pass logs each document they occur in, and at each
  window's end a counting sort groups the log by term into lists that every BOOL query is
  evaluated on, merging with galloping search. Correct on seeds 1 and 2; `ab.py`, 12 runs each,
  two rounds: median 179.82 and 179.46 ms against 183.80 and 183.09 ms, about 2% faster, but
  memory.peak over four runs 52.1–53.9 MB against 41.9–43.0 MB. Of its 58.8 ms of BOOL work,
  logging took 11.5, grouping 8.3 and evaluation 39.0: at least 55 million merge steps, where
  the per-document checks run 16.9 million times.
