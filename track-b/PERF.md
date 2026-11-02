# Track B: performance log

Each kept step has the board run that measured it. Anything marked local was timed on my laptop (an i7-13620H with turbo off), two versions run side by side, and every experiment I reverted sits under the step it was tried after, with its numbers.

| | Reference (step 0) | Final code (step 69), board run |
|---|---|---|
| T | 56.35 s | 87.8 ms (run-11827b164aaf) |
| M | 2,200 MB | 42.0 MB |
| S (lines) | 91 | 947 |
| Points | 0 | 79.5 |

## Checking the headline

Points are 50 · log10 K, where K = (Tref/T)^0.5 · min(Mref/M, 50)^0.2 · min(Sref/S, 1)^0.15 · (Dref/D)^0.15. For run-11827b164aaf: (56.35 / 0.08783)^0.5 = 25.33. Memory is 2,200 / 42.03 = 52.3 times smaller, past the cap of 50, so that factor is 50^0.2 = 2.187. Lines give (91 / 947)^0.15 = 0.704, and with no crates the last factor is 1. K = 39.0, and 50 · log10 39.0 = 79.54. My best B run, run-33959bbbe488 (87.3 ms, step 66, 79.56 points), came before steps 67 to 69, which only cut lines.

## What didn't work

- PGO (under step 31). 161.21 → 168.09 ms median on seed 1, and slower on seed 10, which the profile never saw.
- Step 64. Its one board run read 93.4 ms (run-bb4ed7d47695), in a run where A, C and D also read 1.5 to 3.4% above their best, and I reverted it. Measured again locally it was 2.9% faster, so it went back as step 66 and gave my best B run, 87.3 ms.
- A redesign on the last morning. Before rebuilding B, I measured the most each idea could save (after step 66): an L2-sized term table at most 8.9%, a batched line end about 5%, and starting the pass during planning at most 6 ms. None reached the 10% that would show through the board's noise, so I kept this design.

## How I worked

Every push passed all 524 cases of my gate first, and a change stayed only if the board agreed with my local interleaved runs. When the two disagreed, I measured again before deciding, which is the step 64 story above.

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
| 40 | The plan's checks grouped on a thread of their own while the main thread groups the phrases, flags the terms, numbers the stores and marks the term table; a term's BOOL flag comes from the checks it keys, not from their groups | run-ff1ae2c91791 | 0.1269 | 43.1 | 76.3 |
| 41 | 5.5% fewer instructions in the corpus pass: the walk keys and prefetches the tokens itself, into a buffer whose address and count stay in registers; a block's token ends are taken one line at a time, split at its LFs by one mask, so the token loop tests no LF; a line's id is left out at its end instead of tested for at each token; a key is folded in two vector operations instead of four | run-ff1ae2c91791 | 0.1269 | 43.1 | 76.3 |
| 42 | 7.1% fewer instructions in the corpus pass: the walk's token loop runs in a call of its own per line, so the line end no longer holds the registers it needs, and a key's 32-byte window is tested against a bound held in a register | run-b96f907a19fc | 0.1151 | 41.2 | 77.2 |
| 43 | A phrase candidate is first tested on one more of the phrase's terms, which the plan keeps beside the trigger, before the phrase's terms are loaded: 97.5% of the candidates stop there | run-b96f907a19fc | 0.1151 | 41.2 | 77.2 |
| 44 | The line end's token loop prefetches each token's entry of `check_at`, which the BOOL triggers' loop reads at random, one token ahead of that loop | run-0f0011884453 | 0.1166 | 42.4 | 77.1 |
| 45 | A token whose home slot holds another term stops there unless the slot is marked as the home of a term stored further on; a key's long bit is its byte 12's bit 5, moved by one shift | run-636a838e4422 | 0.1135 | 42.3 | 77.3 |
| 46 | A window's matches counted and summed four words at a time from two nibble tables, instead of six masked popcounts per word | run-f954cb1f35f8 | 0.1117 | 43.3 | 77.4 |
| 47 | A document's phrase triggers are listed in a fifth run of the token loop's slice, by a store and an add, instead of pushed to a `Vec` behind a branch | run-f954cb1f35f8 | 0.1117 | 43.3 | 77.4 |
| 48 | Memory: a chunk's first matches kept as query indices, each doc's followed by its id, 4 bytes per match instead of 8; the long terms' keys kept off transparent huge pages | run-f91848d73390 | 0.1134 | 37.6 | 77.2 |
| 49 | Step 48's first-match list undone, since B's memory sits at the cap; the long terms' keys stay off huge pages, and a term is common from one sampled document in 48 instead of 32 | run-ec587c6dcef4 | 0.1131 | 41.7 | 77.3 |
| 50 | A token's probe carries its home slot's address instead of its index, and a probe past its home slot runs out of line, so the line end's token loop holds no register for the term table and keeps two values on the stack instead of four | run-086c324cb377 | 0.1064 | 41.2 | 77.9 |
| 51 | A probe compares a short key's bytes 8 to 11 with its slot's `tail` as they are, without building the long bit, which only a key past 12 bytes, on its own branch, needs | run-086c324cb377 | 0.1064 | 41.2 | 77.9 |
| 52 | The line end sets a doc's bits in its terms' rows without bounds checks on the term's store or the row | run-9069c200f6e8 | 0.1086 | 42.4 | 77.7 |
| 53 | 37 fewer code lines in code that runs once or once per thread, with the pass's instructions unchanged | run-d502f3e18c27 | 0.1050 | 42.0 | 78.2 |
| 54 | The common terms take ids 1 up, the rows' numbers, so a doc's rows are the first words of its term bits: the line end copies those words, and every 64 docs a 64 × 64 bit transpose turns them into the rows' words; the rows run of `kept`, its store lookups and the `ROW` flag go | run-255e2729d35b | 0.1011 | 42.0 | 78.5 |
| 55 | Reverted after three board runs (see its section). Trial: 11 threads parse the queries while the main thread counts their terms and sets up the term table, then interns each share as soon as it is parsed | run-dc45dd0a1279 | 0.1041 | 41.6 | 78.1 |
| 56 | Trial: the probe's first test compares a key's whole second word with the slot's `tail`, so it takes no branch on a key's length, and a term past 12 bytes is found past that test; the BOOL triggers' loop reads `check_at` and writes `todo` without bounds checks | run-e8e81cdefaa9 | 0.0977 | 41.8 | 78.8 |
| 57 | The id lists' entries pushed to the end of one log, and sorted into the lists by counting at each window's flush, instead of pushed into one of 3,452 lists at random | run-e8e81cdefaa9 | 0.0977 | 41.8 | 78.8 |
| 58 | Trial: of the short terms whose home slot a slot is, the one with the most sample hits moves into it, on a thread of its own during the plan's tail, so 44% fewer probes go past the home slot | run-f39c6987110f | 0.0981 | 42.3 | 78.6 |
| 59 | Trial: the probe's first test branches on one select, the key's difference kept only when the slot has `MORE`, and the token loop at the line end no longer prefetches each token's `check_at` entry, so it keeps every address in a register | run-b6d1d5065964 | 0.0950 | 41.7 | 79.0 |
| 60 | While the main thread interns the terms, three threads read the corpus a byte per 64 KiB, so the scanning threads' walks take no page faults on it; the chunks are freed on a thread of their own while the output is written | run-938dc27813b7 | 0.0952 | 41.3 | 79.0 |
| 61 | The line end's token loop holds a zero in a register for its three selects, and the tokenizer takes a long token's key on a cold branch, the mask's address from the length negated, and a token's start and end as offsets from its block: 3.9 fewer instructions per token | run-280052ffeb2c | 0.0979 | 41.8 | 78.6 |
| 62 | Reverted after two board runs (see its section). Trial: the line end's token loop stores each token's flags only, and a doc's terms kept in lists, BOOL triggers and phrase triggers are packed into their runs 8 tokens at a time by a permutation table, 10 instructions fewer per token in the loop for 4.25 in the packing; a key's home slot comes from its first word alone, two instructions fewer per token in the tokenizer | run-e89cc599f8ef | 0.1054 | 40.9 | 77.7 |
| 63 | Trial: step 62's packing of a doc's runs, 8 tokens at a time, with step 61's hash of the key's first two words | run-51792bbbc94f | 0.0929 | 41.9 | 79.1 |
| 64 | Reverted after one board run (see the not-kept list). Trial: the query terms interned on every thread into one shared table, a new term's slot claimed by a compare-and-swap of its `head` and its id taken from a block of 64; the page-touching helpers moved into the plan's tail | run-bb4ed7d47695 | 0.0934 | 41.7 | 78.8 |
| 65 | Reverted after two board runs (see the not-kept list). Trial, pushed as 052ab6e: 32-byte aligned checks, a branch-free `add_checks` and immediate vector shifts in `pack`, S 901 → 956 | run-e40ad8747964 | 0.0944 | 41.7 | 78.7 |
| 66 | Step 64's parallel intern again, on step 63's code (see its section) | run-33959bbbe488 | 0.0873 | 42.1 | 79.6 |
| 67 | 4 fewer lines: `group_into`'s two counting loops as one-line `for_each` calls. Below board resolution | run-2655357b685f | 0.0884 | 41.8 | 79.4 |
| 68–69 | 10 fewer lines: the six check masks computed by `has(k)`; the intern's slot claim as one `let`, its two stores as one-line statements. Below board resolution | run-11827b164aaf | 0.0878 | 42.0 | 79.5 |

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

Locally, step 2 → 3a → 3b: elapsed 0.584 to 0.592 → 0.469 to 0.534 → 0.427 to 0.429 s; cycles 13.28 to 13.56 →
9.97 to 10.08 → 10.04 to 10.09 G; memory.peak 173.3 → 82.1 → 79.7 MB. S went from 484 to 745.

## Step 4: the serial start

Before the pass, one thread parsed the queries and built the plan while the other eleven
waited: 56 to 59 ms of a 0.43 s run.

- **Parse, 32 → 15 ms.** Every thread parses a share of the query lines into flat arrays. One
  thread then gives each distinct term its id, in file order, so the ids do not depend on the
  thread count. It loads the table slot of the term 16 ahead while it interns one, so the
  cache misses overlap, and when the table doubles it is rebuilt from the keys in id order.
- **Plan, 25 → 15 ms.** A query's terms and operators sit in flat arrays instead of two `Vec`s
  per query. The phrases and checks each term triggers are collected as pairs and grouped by
  one counting sort, instead of a `Vec` per term.

Locally, step 3 → 4: elapsed 0.4335 to 0.4380 → 0.3905 to 0.3929 s; cycles 9.97 to 10.05 → 9.84 to 9.87 G;
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
  68.3 → 50.8 MB, cycles 9.91 to 9.97 → 9.85 to 9.91 G.
- **A fixed mmap threshold.** glibc raises its mmap threshold to the largest mapped block freed
  so far, and parsing and planning free blocks of up to a few MB, so smaller blocks then come
  from per-thread arenas and stay resident after they are freed. With
  `mallopt(M_MMAP_THRESHOLD, 128 KiB)`, every block of 128 KiB or more goes back to the kernel
  when it is freed, such as each thread's first matches once they are grouped. In the first
  measurement: 50.8 to 51.2 → 42.4 to 45.3 MB at the same speed.

Locally, step 4 → 5, six interleaved rounds on a busy machine: cycles 9.74 to 9.89 → 9.73 to 9.89 G,
elapsed 0.3926 → 0.3867 s in the quietest round; memory.peak 68.4 to 69.8 → 39.6 to 40.8 MB over five
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
local, turbo off; step 5 ran 384 to 387 ms in the same sessions):

1. **Truth tables.** Each per-document BOOL query has at most 6 distinct terms, so it compiles to
   a 64-bit truth table: its AND, OR and NOT applied to six fixed bit masks. The skip for
   documents already counted from an earlier cover term is folded into the table. A check is a
   shift and a mask. 350.5 ms; branch misses 117 → 80 M.
2. **One tally per check.** Each check keeps its count and id sum in one u64 next to its
   neighbours, and the check and its tally are prefetched when the trigger term is first seen
   in a document. 320 ms.
3. **A mask table for the key fold.** A sliding table of 0xFF bytes replaces the u128 shifts,
   `key_in` inlines, and 2.2 G instructions disappear. 288.5 ms, but memory rose 8 to 10 MB.
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

Locally, step 6 → 7, three interleaved rounds: cycles 7.07 to 7.23 → 6.89 to 7.01 G, task-clock
3,059 to 3,131 → 2,958 to 3,014 ms, elapsed 0.2997 to 0.3045 → 0.2838 to 0.2843 s in the two quiet rounds.
Alternating runs, 20 each: median wall 295.2 → 285.3 ms. memory.peak over eight runs each:
41.1 to 44.2 → 40.2 to 45.1 MB (medians 41.9 and 43.3). S went from 837 to 871.

## Step 8: the term table sized once

The intern grew the term table from 1,024 slots by doubling, which rebuilt it eight times. The
table is now created with one slot per term occurrence in the queries, rounded up to a power of
two (about 153K occurrences and 75.7K distinct terms on all seven seeds, so 262,144 slots, the
size the doubling reached anyway). The intern drops from about 11 to 6.5 ms. The keys are trimmed
to their length once the intern ends, since the spare capacity would stay resident on huge pages.
The scan's machine code is unchanged.

Local, the time from start to the end of planning, medians of 6 interleaved runs: 27.96 → 23.5
ms. User instructions 10.23 → 10.06 G. memory.peak 42.2 → 42.6 MB (noise about ±1 MB). S
unchanged. Gate: 507 pass, 0 fail; full seeds 1 to 7 match.

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
interleaved rounds, cycles 7.07 to 7.10 → 6.81 to 6.84 G, task-clock 3,039 to 3,054 → 2,931 to 2,943 ms,
elapsed 0.2858 to 0.2864 → 0.2781 to 0.2791 s in the two quiet rounds. Alternating runs, 20 each:
median wall 286.0 → 277.3 ms (step 6: 297.9). S unchanged.

## Step 10: huge pages for the term table only

A huge page is resident as a whole, so step 7's allocator cost memory for every large block that
only partly fills its last huge page: on the board, 43.1 → 46.3 MB, and still 44.2 MB at step 9.
The term table is exactly two huge pages and the pass reads all of it, so it is the one block whose
huge pages cost no memory. It is now allocated alone at a 2 MiB boundary and advised once, and the
34-line global allocator goes, with step 8's trim of the keys, which only mattered on huge pages.
Walk cycles: 427 to 517 M with no huge pages, 136 M at step 9, 246 to 257 M now.

Locally, against step 9, three interleaved rounds: cycles 6.87 to 6.90 → 6.85 to 6.90 G, task-clock
2,952 to 2,967 → 2,969 to 2,989 ms, elapsed 0.2755 to 0.2770 → 0.2770 to 0.2791 s. Alternating runs, 14 each,
twice: median wall 275.6 and 276.9 → 276.8 and 278.7 ms (no huge pages at all: 282.5 and 282.2).
memory.peak over six runs each: 41.3 to 44.9 → 41.2 to 43.8 MB; this machine does not show the
board's difference. S 871 → 849. Gate: 507 pass, 0 fail.

## Step 11: no branch on a check's result

A check adds `hit` times `1 << SUM_BITS | doc` to its cell ([`Tally::add_check`]), and the
compiler made that product a branch on `hit`. Whether a document matches a check follows no
pattern, so the branch missed 4.2 M times, 6% of all branch misses. `std::hint::select_unpredictable`
makes it a conditional move.

Locally, against step 10, three interleaved rounds: branch misses 68.2 → 64.0 M, cycles
6.80 to 6.84 → 6.61 to 6.68 G, task-clock 2,948 to 2,966 → 2,864 to 2,896 ms, instructions 10.06 → 10.22 G.
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
6.60 to 6.67 → 6.16 to 6.25 G, task-clock 2,858 to 2,893 → 2,669 to 2,710 ms. Alternating runs, 16 each,
twice: median wall 272.0 and 270.5 → 255.4 and 254.0 ms (step 6: 297.5 and 295.6). S 849 → 846.
Gate: 507 pass, 0 fail.

## Step 13: one multiply per token hash

The hash of a token's key folded its four words with a multiply each, a chain of four dependent
multiplies plus four extractions from the vector register, on every token. It now XORs the words
in the vector register and multiplies once. Equal XORs of different keys only share a probe
sequence, which the `head` compare still tells apart, and the slots still come from the top
bits of the product.

Locally, against step 12, three interleaved rounds: instructions 8.86 → 8.64 G, cycles
6.18 to 6.19 → 6.01 to 6.04 G, task-clock 2,677 to 2,681 → 2,609 to 2,623 ms. Alternating runs, 16 each,
twice: median wall 253.0 and 255.4 → 247.4 and 249.0 ms. S 846 → 844. Gate: 507 pass, 0 fail.

## Step 14: the first-match counts in bytes

A check's first ten matches in a chunk also go to its query's list of first matches, up to ten per
query, and the query's own cell counted them. That cell is one of 50,000 u64s per thread, 400 KB,
read at random on 7 M early hits, and mostly from L3. The count now sits in a byte per query
([`Tally`]), 50 KB per thread, which stays in cache. The query's cell is no longer touched during
the pass, so it no longer needs zeroing before the checks' cells are added in.

Locally, against step 13, three interleaved rounds: cycles 6.05 to 6.15 → 5.94 to 6.00 G, task-clock
2,623 to 2,669 → 2,582 to 2,607 ms. Alternating runs, 16 each, twice: median wall 247.9 and 247.3 →
245.4 and 244.8 ms. memory.peak over four runs: 42.2 to 46.2 → 43.7 to 44.6 MB (the bytes add 0.6 MB
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

Locally, memory.peak over seven runs each: 43.1 to 45.4 MB (median 44.7) → 41.5 to 42.5 MB
(median 41.8). The window alone: median 41.9, but one run at 44.9. Cycles and wall unchanged:
5.94 to 5.96 → 5.93 to 5.94 G; alternating runs, 12 each: median 246.0 → 245.1 ms. S 845 → 848.
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

Locally, against step 15: page faults 18,800 → 16,710, kernel cycles 214 → 156 to 157 M, user
cycles unchanged (5.95 to 6.00 → 5.88 to 5.99 G). Alternating runs, 16 each, twice: median 246.4 and
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
rise a little, 171 to 182 → 178 to 192 M in a noisy session (the same work, spread over cores, plus
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

Locally, against step 17: user cycles 5.85 to 5.89 → 5.68 to 5.70 G over five rounds of `perf stat
-r 5`, instructions 8.573 → 8.687 G (the extra compare). Alternating runs (`ab.py`), 15 or 16
each, five rounds: median 237.3, 237.9, 237.8, 237.2 and 237.1 → 229.8, 229.8, 230.6, 229.5 and
229.5 ms (-3.2%). memory.peak over six interleaved runs: median 43.2 → 43.0 MB (the table is
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
and 230.8 → 226.8 and 226.7 ms. `perf stat`: user cycles 5.59 → 5.51 to 5.56 G, instructions 8.739 →
8.686 G. In a `perf record` timeline the pass starts at 21.4 ms instead of 21.1 to 22.0 ms, and
its threads' cycles fall 5.59 → 5.51 G. memory.peak over six interleaved runs: median 41.2 →
41.0 MB. S 758 → 771. Gate: 510 pass, 0 fail.

## Step 21: 35 fewer lines, loops on 64-byte boundaries

Cuts in code that runs once per run or once per thread:

- **One `parallel` helper** (7 lines) runs `work(i)` for each `i` on a thread of its own and
  returns the results in order. The pass, the output and the query parse each spelled out a
  `thread::scope` in 8 to 12 lines; each now takes 2 to 4 (-15 in all). The sample keeps its
  own: it adds each part as its thread is joined, and collecting every part first made the
  sample 0.08 to 0.19 ms longer (min and median of 15 timed runs).
- **The intern loop without its prefetch 16 terms ahead** (-5). The serial end of the parse
  (table allocation, merge and intern of seed 1's 152,904 term occurrences) took 5.49 ms with
  it and 5.48 ms without, median of 15 timed runs each.
- **The output** adds a query's cells over all chunks and splits the total once (-2): together
  the chunks' id sums stay below `2^SUM_BITS`, as each one does, since `answer_all` bounds the
  sum of all ids. The first matches come straight from the chunks through
  `flat_map(..).take(SHOWN_IDS)` instead of a buffer, and `Chunk`'s fields are `at` and `ids`,
  so its literal fits on one line (-8).
- **`Plan::new`** numbers a stored term's bitset row or id list from one two-entry counter
  (-4), and **`line_bounds`** takes its last bound from the loop: a cut at `docs.len()` finds
  no LF after it (-1).

**Loops on 64-byte boundaries.** Built as before, this step ran slower than step 20 though it
adds no work: alternating runs (`ab.py`, 61 each), two rounds, median 226.4 and 226.2 → 227.2
and 227.0 ms. Each cut alone on step 20, two rounds: the helper +0.41 and +0.38 ms, the intern
loop +0.07 and -0.13 ms, the rest +0.88 and +0.85 ms. The cost is where the hot loops sit. The
`Plan::new` cut alone, which runs once and leaves the instruction count at 8,686.1 M, moved the
scan thread's code, byte for byte the same, from offset 16 of a 64-byte line to offset 0, and
user cycles rose 0.3 to 0.6%. `-C llvm-args=-align-loops=64` in `.cargo/config.toml` starts every
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
  line (-5).
- `bit_index_sum` names its per-mask closure, `weight` (-1).
- `TermTable::new` binds the empty `keys` first, so the struct literal fits on one line (-2).

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
214.20 and 213.85 ms. `perf stat`: user cycles 5.64 → 5.30 to 5.31 G, instructions 8,711 → 7,517
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
rounds, run while the box was loaded (load average 7 to 8), are left out. `perf stat`: branch
misses 52.1 → 20.8 M, instructions 7,625 → 8,392 M, user cycles 5.23 → 4.42 to 4.44 G.
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
`perf stat`: user cycles 4.39 to 4.44 → 4.31 G, instructions 8,392 → 8,236 M, branch misses 20.8
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

Timeline over three instrumented runs: the loop 4.9 → 1.0 to 1.1 ms, adding the flags 1.5 → 0.5 to 0.7
ms, and the pass starts at 16.7 to 17.2 ms instead of 20.7 to 21.7. Locally, against step 26
(alternating runs, `ab.py`, 18 each), two rounds: median 178.38 and 178.94 → 174.46 and 174.79
ms, min 177.18 and 177.43 → 173.36 and 173.27 ms. `perf stat`: user cycles 4.30 to 4.31 → 4.31 G,
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

Timeline over three instrumented runs: the intern 4.7 to 5.0 → 3.9 to 4.3 ms, the sample's end 0.3 →
0.05 ms, the sample 4.8 to 5.1 → 3.4 to 4.0 ms (a third run of step 27, slowed by other load, took
6.5). Locally, against step 27 (alternating runs, `ab.py`, 20 each), three rounds: median
174.96, 175.52 and 175.31 → 173.39, 173.52 and 173.54 ms, min 174.24, 173.97 and 172.99 →
172.68, 172.50 and 171.83 ms. `perf stat`: user cycles 4.32 to 4.34 → 4.34 to 4.36 G, instructions
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
`ab.py`, 18 to 20 each, with other jobs on the box, load average 4 to 7), three rounds: median
173.14, 174.52 and 174.12 → 168.32, 168.92 and 169.58 ms, min 171.99, 172.55 and 172.64 →
167.31, 167.11 and 167.60 ms. `perf stat`: user cycles 4.30 to 4.31 → 4.15 G, instructions 8,263
→ 8,261 M (the sum saves about 310 M, the second list adds as many), branch misses 20.5 →
18.3 to 18.4 M. memory.peak over eight alternating runs: median 41.3 → 40.8 MB. S 746 → 745.
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
average 7 to 11), three rounds: median 169.99, 172.52 and 169.26 → 158.49, 159.94 and 158.42 ms,
min 167.46, 166.78 and 167.99 → 156.55, 157.30 and 156.29 ms. `perf stat`: user cycles 4.01 →
3.62 G, instructions 8,268 → 7,338 M, branch misses 18.2 → 18.1 M. memory.peak over eight
alternating runs: median 41.3 → 40.9 MB. S 745 → 746. Gate: 510 pass, 0 fail; seeds 2, 3, 7,
10 and 12 match their reference hashes.

Taking `seen` as a slice inside the closure cut 81 M more instructions but did not measure. A
flag in the term table marking a home slot that a term was displaced from, so that a probe
ends at its home slot unless the flag is set, sent 1.39 million of the 36.8 million lookups
past the home slot and cut branch misses from 18.4 to 15.6 M, but it added 91 M instructions,
the user cycles stayed at 4.13 to 4.16 G, and the wall did not move: 169.69 → 169.10 and 168.87
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

Local, against step 31 (`ab.py`, 20 alternating runs, load average 2 to 6), four rounds: medians
156.66, 157.64, 157.21 and 156.48 → 152.23, 152.33, 153.10 and 152.11 ms, minimums 155.37,
155.21, 155.82 and 155.02 → 151.04, 151.06, 150.88 and 150.80 ms. `perf stat`: user cycles 3.82
→ 3.69 G, instructions 7,338 → 7,257 M, branch misses 18.1 → 16.9 M. memory.peak over five
alternating runs: median 41.3 → 41.3 MB. S 745 → 750. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10
and 12 match their reference hashes.

Moving the loop into the method alone, with the branch on `hit` kept, cut instructions to
7,202 to 7,270 M but left branch misses at 18.1 M and user cycles at 3.79 to 3.82 G, inlined or not.
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
6,980 M and 3.56 to 3.64 G. Branch misses stayed at 16.8 to 16.9 M. `ab.py`, 18 alternating runs, two
rounds, step 33 / the probe unchecked / all three: medians 152.84 / 151.19 / 150.65 and 152.63 /
150.84 / 150.26 ms, minimums 150.77 / 149.93 / 149.18 and 151.52 / 149.94 / 149.35 ms.

Local, against step 33 (`ab.py`, 20 alternating runs, load average 3 to 5), three rounds: medians
153.42, 153.74 and 152.45 → 151.11, 151.83 and 151.14 ms, minimums 151.91, 152.08 and 151.64 →
149.87, 148.39 and 149.27 ms. `perf stat`: user cycles 3.69 → 3.56 to 3.65 G, instructions 7,257 →
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
for the rows and lists 6,826 M, with user cycles 24 to 54 M lower in two rounds; phrases inline
6,751 M, with branch misses 17.0 → 17.6 M; the checks in a sized slice 6,556 M, branch misses
16.6 M; the window with one bounds check 6,444 M; the two-word hash 6,370 M; the carried token
picked once per block 6,179 M.

Against step 34 (`perf stat -r 3`, three alternating rounds; battery, not comparable): user
cycles 3,661 to 3,672 → 3,361 to 3,391 M, instructions 6,980 → 6,178 M, branch misses 16.9 → 16.6 M.
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

After step 35 the pass started 14.2 to 14.9 ms into a run on seed 1 (local, on AC, turbo off, a
build with timestamps, 12 threads on the P-cores, medians over three rounds of 11 or more
runs). Interning the query terms on one thread took 3.7 ms, the sample 3.4 to 3.7 ms and the
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
  and the keys filled first: the planning 1.30 to 1.34 → 0.98 to 1.06 ms.
- **The sample in one cell per term.** [`count_hits`] kept, per term, a count and the last
  document counted in two arrays, so each token cost two random accesses, and the second
  array, 606 KB per thread, took 148 page faults on each. A `u32` cell now holds the count in
  its high half and the last document's number in its low half: the sample 3.40 to 3.72 →
  2.33 to 2.46 ms.
- **The flags in bytes.** The flags that [`TermTable::mark`] adds to the slots are collected
  in bytes instead of `u32`s: 0.15 to 0.16 → 0.04 ms.

In the round that timed both, the pass started at 14.24 ms for step 35 and 12.06 ms for this
step; over three rounds each, at 14.24 to 14.86 and 12.06 to 12.50 ms, and the pass itself took
118.8 to 121.6 and 119.5 to 119.8 ms.

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
terms, or in a cold function, instead of in an array by id, added 200 to 302 M instructions to the
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
  huge pages, the call returned in 33 to 172 µs in three runs. With THP off while the table is
  written and back on before the call, the collapse took 1.38 to 1.48 ms and gave back the counts
  with THP: cycles 3,521 M, dTLB load misses 2.50 M, memory.peak 41.1 MB. With THP off for the
  whole process, it fails and changes nothing: cycles 3,736 M.

In 12 alternating runs each, neither step 36 nor this step lost the table's huge pages on this
machine, although 5% of its THP faults since boot fell back to 4 KiB pages and 91% of its direct
compactions failed (`/proc/vmstat`). So the collapse helps where the faults could not, or did
not try to, compact memory, and costs a fraction of a millisecond elsewhere.

Local, against step 36 (`ab.py`, 20 alternating runs on the P-cores, on AC, load average 4 to 6),
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
  queries, still dropped. After step 33 the same leak, plan, chunks and queries, measured -0.96,
  +0.10 and -1.80 ms in three rounds of 20 runs, too noisy to keep it.

- **Numbers by `format_into`.** `format_into`, with a `NumBuffer`, writes a number's digits
  without the formatting machinery of `write!`. Each number goes out followed by a space, and
  the line's last space becomes its LF: formatting 1.83 → 1.44 ms.

In the timing build, a run took 135.65 → 134.14 ms (median of 8).

Local, against step 37 (`ab.py`, 20 alternating runs on the P-cores, on AC, load average 4 to 6),
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
  alternating rounds: dTLB load misses 2.49 to 2.59 → 0.41 to 0.47 M, dTLB store misses 0.28 to 0.30 →
  0.21 M, page faults 16,646 to 16,674 → 16,237 to 16,268, cycles 3,401 to 3,502 → 3,444 to 3,455 M,
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

Local, against step 38 (`ab.py`, 20 alternating runs on the P-cores, on AC, load average 2 to 6),
six rounds: medians 137.54, 137.87, 136.85, 137.32, 137.11 and 137.23 → 134.96, 135.30, 136.33,
135.57, 135.05 and 135.66 ms, minimums 135.87, 136.16, 135.55, 136.13, 136.14 and 136.46 →
133.38, 134.02, 134.19, 134.09, 133.95 and 134.47 ms. memory.peak over 28 alternating runs:
median 41.45 → 41.59 MB, highest 42.84 → 43.72 MB, as the checks' block takes 2.10 MB where
they took 1.63. S 763 → 769. Gate: 510 pass, 0 fail; seeds 2, 3, 7, 10 and 12 match their
reference hashes.

## Step 40: the checks grouped while the table is marked

After step 39, once its threads have planned the queries, the plan took 2.45 ms more on the
main thread alone (a build with timestamps, medians of 10 runs): grouping the phrases 0.30 ms,
the checks' block and the checks' grouping 0.36 and 0.79, the term flags 0.06, the stores 0.26,
marking the term table 0.50 and the frees 0.19.

The checks now go to a thread of their own, which allocates their block and groups them, while
the main thread groups the phrases, flags the terms, numbers the stores and marks the table.
Marking needs every term's flags, so a term's BOOL flag now comes from the checks' lists, set
for each check's key, instead of from the grouped checks. In the same build, the plan's tail
takes 1.76 ms instead of 2.45: the helper is done 1.36 ms after it starts, its block taking
0.49 ms and its grouping 0.88, while the main thread groups the phrases in 0.51 ms, spawn
included, flags the terms in 0.29, numbers the stores in 0.27 and marks the table in 0.52, so
the join waits for nothing. The walks begin at 14.19 ms instead of 14.89.

Local, against step 39 (`ab.py`, 20 alternating runs on the P-cores, on AC, load average 4 to 8),
six rounds: medians 135.68, 135.60, 135.16, 135.64, 135.55 and 136.89 → 134.71, 134.42, 134.22,
135.07, 134.36 and 135.32 ms, minimums 134.42, 134.50, 133.46, 133.46, 133.90 and 134.23 →
133.51, 133.10, 132.99, 132.92, 132.88 and 133.19 ms. memory.peak over 40 alternating runs:
median 41.62 → 41.76 MB, highest 43.34 → 44.33 MB, the next highest 43.36; nothing allocated
changes, only the thread that allocates the checks' block. S 769 → 774. Gate: 510 pass, 0 fail;
seeds 2, 3, 7, 10 and 12 match their reference hashes.

## Step 41: the walk's probes in registers, and a block's tokens split at its LFs

After step 39 the pass ran 6,132 M instructions on seed 1, and two loops took most of its
cycles: the walk, which keys, hashes and prefetches each token (28% of the samples), and the line
end's token loop (34%). The walk loaded eight values from the stack at each token, three of them
the header of the `Vec` of probes, whose length it also stored back at every push.

- **The walk keys the tokens.** Both callers gave [`walk`] the same token callback, a key and a
  prefetch pushed to a `Vec`. The walk now does that itself and passes each line's probes to its
  one callback as a slice. They sit in a buffer that each block first grows, if it must, to 32
  entries past the line's tokens so far, the most that one block ends, so the buffer's address
  and the count stay in registers and the push tests no capacity: 5 instructions instead of 11.
- **The ids left out at the line end.** Every line starts with its id, which reads as a token.
  The walk compared every token's start with the line's start to skip it; the line end now takes
  the line's tokens from the second on.
- **A block's ends split at its LFs.** The token loop tested at each token whether an LF came
  before it. A token never spans an LF, so the block's ends are now taken one line at a time:
  those up to the next LF, its own bit included, with one mask (`lfs ^ (lfs - 1)`, which takes
  all of them when the block holds no LF). The token loop tests no LF, and the line end, now
  called from one place, is inlined into the walk.
- **The key fold in two steps.** A key ORs 0x20 into each of its 32 bytes, which lowercases a
  letter and leaves a digit as it is, and then masks off the bytes past the token: two vector
  operations instead of four.

Built up on step 39 (`perf stat -r 5`, two rounds): the fold, instructions 6,132.1 → 6,053.5 M,
cycles 3,270 and 3,282 → 3,267 and 3,267 M; the walk keying the tokens, 5,936.5 M, 3,199 and
3,199 M; the ids left out, 5,886.6 M, 3,164 and 3,219 M; the ends split at the LFs, 5,794.5 M,
3,167 and 3,165 M, and branch misses 16.63 → 16.29 M.

Local, against step 39 (`ab.py`, 41 alternating runs on the P-cores, on AC, load average 5 to 6),
two rounds: medians 135.79 and 135.36 → 132.35 and 131.44 ms, minimums 133.76 and 133.45 →
129.75 and 130.03 ms. memory.peak over 8 alternating runs: median 41.42 → 40.71 MB, highest
42.97 → 42.06 MB. S 774 → 763 (on step 39's code, 769 → 758). Gate: 510 pass, 0 fail.

The board ran steps 40 and 41 together, run-ff1ae2c91791 (commit f17de2c), 126.9 ms; a second run of
the same code, run-9f07c5db9176 (commit ca86ea9, which changed only track C), drew 124.4 ms, 42.7 MB
and 76.5 points.

## Step 42: the walk's token loop in a call of its own

After step 41, the walk's token loop, inlined into `scan`'s loop together with the line end,
kept four of its values on the stack: the block's token starts, the hash's shift, the slots'
address and the chunk's address. It loaded them at every token and stored the starts back, 38
instructions per token, as the line end's values held the registers across the whole loop
nest.

- **A call per line.** `Tokens::line` runs the token loop up to the next line end, never
  inlined, and returns the line's token count; the walk keeps the block's state between lines
  (its offset, its token starts, token ends and LFs not taken yet, the next token's start, and
  whether it ends in a token), 250,000 calls on seed 1. In a function of its own, the token loop
  keeps every value in a register: 31 instructions per token, none of them a stack access.
- **The window's bound in a register.** `chunk.get(start..start + MAX_TERM)` also tested whether
  `start + MAX_TERM` wraps around. LLVM had ruled that out while the block's offset came from a
  loop over the chunk, but not once it comes from the walk's state: 6 instructions per token
  for the test. `key_in` now tests `start < chunk.len() - 31` (saturating), against a bound
  computed once: 2 instructions.
- The walk ends where the next block would start past the chunk, which ends in LF, so it needs
  no test of its own for the last line.

Built up on step 41 (`perf stat -r 5`): the call per line, instructions 5,796.0 → 5,517.0 M,
cycles 3,182 → 3,070 M; the bound in a register, 5,373.9 M, and cycles 3,109 → 2,997 M in that
round; as committed, 5,382.2 M. Against step 41, two rounds: instructions 5,796.0 → 5,382.2 M,
cycles 3,132 and 3,135 → 2,961 and 2,972 M, branch misses 16.28 → 16.32 M.

Local, against step 41 (`ab.py`, 41 alternating runs on the P-cores, on AC, load average 4.7 to 7.7
with a browser taking half of one P-core, so the minimums are the steadier figures), two rounds:
medians 132.05 and 137.28 → 127.15 and 132.88 ms, minimums 129.20 and 129.46 → 124.29 and
124.14 ms. memory.peak over 8 alternating runs: median 41.25 → 40.71 MB, highest 42.02 → 41.94
MB. S 763 → 777. Gate: 510 pass, 0 fail.

## Step 43: a phrase candidate tested on one more term first

After step 42, the phrase triggers of seed 1's documents led to 1,028,628 (trigger, phrase)
candidates, of which 19,427 match, 1.9%. Each loaded its phrase's bounds and terms from
`Queries`, two arrays read at random, and compared the terms in a loop whose length varies: 4.1%
of the cycle samples and 5% of the mispredicted branches.

The plan now keeps, beside each phrase's query and the trigger's position in it, the position
and id of one more of its terms: the first, or the second when the trigger is the first. The
pass compares that term with the document's term at the same place, which its own list of
terms holds, and loads the phrase only when they agree: for 25,843 candidates, 2.5%. The plan's
phrase entries take 16 bytes instead of 8, 304 KB in all.

Against step 42 (`perf stat -r 5`, two rounds): instructions 5,382.2 → 5,339.3 M, cycles 2,961
and 2,972 → 2,875 and 2,877 M, branch misses 16.32 → 15.48 M.

Local, against step 42 (`ab.py`, the rounds of step 42): medians 127.15 and 132.88 → 125.81 and
124.51 ms, minimums 124.29 and 124.14 → 120.68 and 120.62 ms. memory.peak over 12 alternating
runs: median 41.45 → 41.80 MB, highest 42.50 → 43.21 MB. S 777 → 782. Gate: 510 pass, 0 fail.

## Step 44: each token's checks prefetched

After step 43, the BOOL triggers' loop at a line's end stalled on its load of `check_at[t]`, the
start of the trigger's range of checks: a 303 KB array read at random, 3.9% of the cycle samples.
The line end's token loop, which finds each token's slot in the term table, now prefetches that
token's entry, so it has started to load by the time the triggers' loop reads it. The same
prefetch did not pay on step 41 (see below); since step 42 the token loop runs in a call of its
own and no longer spills for it.

Against step 43 (`perf stat -r 5`, three rounds): instructions 5,339.3 → 5,412.1 M, cycles 2,909,
2,909 and 2,912 → 2,832, 2,893 and 2,863 M, branch misses 15.47 → 15.46 M.

Local, against step 43 (`ab.py`, 41 alternating runs, four rounds, the first two on a build that
differs only in line numbers): medians 122.97, 122.00, 121.81 and 121.81 → 121.92, 121.03, 121.53
and 120.87 ms, minimums 120.28, 120.95, 120.08 and 120.67 → 119.65, 119.76, 119.83 and 119.23 ms.
memory.peak over 8 alternating runs: median 41.88 → 42.38 MB, highest 43.71 → 43.15 MB. S 782 →
783. Gate: 510 pass, 0 fail.

## Step 45: the probe stops at a taken home slot

After step 44, `perf record` on seed 1 (turbo off): 15.46 M mispredicted branches, `tma_bad_speculation`
17.1% of the slots, and 22% of those mispredicts on the probe's first test, the one branch that
ends a probe at the term or an empty slot. A count of where each token's probe ends: of the
7.56 M tokens that are no query term, 5.39 M found their home slot empty and 2.17 M found it
taken (1.33 M one slot on, 0.49 M two, 0.35 M three or more); of the 28.72 M term tokens, 27.63 M
found their term at home and 1.08 M further on. So the 3.25 M probes that went on past their
home slot mispredicted, and two thirds of them belonged to tokens that are no term at all.

- **`Slot::MORE`.** `intern` sets it on a term's home slot when it stores the term further on,
  and a term met again keeps it. At the home slot, [`TermTable::find_from`] stops when the slot
  holds the key or has no `MORE`, in one branch, as the probe's own test is, and returns flags
  of 0, the empty slot's, when it stopped at another term. An empty slot has no `MORE`, so that
  test also stops at it. Only a slot that is some displaced term's home sends a probe on: 4.5%
  of the slots at the table's 29% load, instead of 29%. A slot's flags hold the term id times
  32 now, so a term id stays below 2^27.
- **`#[inline(always)]`.** With its first slot peeled off the loop, `find_from` was no longer
  inlined into the pass or the sample: instructions 5,412 → 6,629 M, cycles 2,900 → 3,150 M.
  Cargo.toml allows clippy's `inline_always` for it, which costs no line.
- **The long bit in one shift.** `Slot::tail` set its top bit from `key[1] >> 32 != 0`, six
  instructions per token. A key's every byte has bit 5 set ([`fold`]), so bit 37 of `key[1]` is
  set exactly when the key runs past 12 bytes, and `(key[1] >> 6) as u32 & 1 << 31` moves it
  into place; the long test is that bit too, and the long key's load takes no bounds check.

`perf stat -r 5`, interleaved, two rounds, step 44 → `MORE` → both: instructions 5,412.1 →
5,466.2 → 5,360.1 M, cycles 2,902 and 2,897 → 2,811 and 2,804 → 2,778 and 2,784 M, branch misses
15.46 → 12.56 → 12.57 M. Hashing the key's first word only, instead of the XOR of two, would save
another 72 M instructions, but put more terms off their home slot: branch misses 12.57 → 12.74
M and cycles 2,747 → 2,763 M, so it did not stay.

Local, against step 44 (`ab.py`, 41 alternating runs on the P-cores, turbo off), `MORE` alone,
two rounds: medians 120.83 and 120.27 → 117.71 and 117.45 ms, minimums 118.79 and 118.76 →
116.47 and 116.14 ms. Then step 44, `MORE` alone and both, two rounds: medians 120.62 and 120.62
→ 117.33 and 117.42 → 116.42 and 116.37 ms, minimums 118.63 and 119.33 → 116.19 and 116.09 →
115.47 and 114.82 ms. As committed (a build that differs only in line numbers), against step 44,
two more rounds under a higher load: medians 123.80 and 123.09 → 119.52 and 118.77 ms, minimums
121.34 and 120.32 → 117.37 and 116.13 ms. memory.peak over 6 alternating runs: median 39.1 → 39.9 MB, highest 40.9 →
40.0 MB, with the same allocations. S 783 → 800. Gate: 510 pass, 0 fail.

## Step 46: a window's sums from nibble tables

After step 45, `Window::flush` took 5 to 7% of the cycle samples, and about half of that went to the
sum in `Tally::add_bits`: per word of a query's window, a popcount for its count and six masked
popcounts for the sum of its bits' positions (`bit_index_sum`), 1,773 queries on sets times 61
windows. LLVM vectorized them with `pshufb`, a table lookup per nibble for each of the seven.

[`window_cell`] does the window in 16 steps of four words, with AVX2. Two lookups per nibble give
each byte's bit count and the sum of its bits' places in the byte (a third table, for the high
nibble, adds 4 per bit to the second). `sad` adds the counts and the places per word, and
`maddubs` and `madd` weigh each byte's count by its offset in the word, 0 to 56, leaving each
word's total in two 32-bit halves. A multiply adds each word's offset in the window times its
count, and the window's first id times the whole count comes in at the end. The window's
positions add up to less than 2^32, so the halves never carry into each other. The function is
unsafe, as `Check::matches` and `classify` are, and takes one SAFETY comment at its call.

Against step 45 (`perf stat -r 5`, interleaved): instructions 5,360.1 → 5,264.2 M, cycles 2,763
and 2,769 → 2,656 and 2,677 M on a first form of the same code, and 2,770 → 2,730 M as committed;
branch misses unchanged (12.6 M). Part of the cycles saved come from the vector ports that the
other hyperthread's token loop leaves idle, so the wall moves less than the cycles.

Local, against step 45 (`ab.py`, 41 alternating runs, on a box with a load average of 3 to 4 from
other work): the first form, four rounds, medians 122.41, 118.78, 119.17 and 119.52 → 120.71,
117.86, 117.57 and 117.99 ms, minimums 117.65, 117.41, 117.05 and 116.87 → 117.06, 116.02, 115.58
and 115.99 ms; as committed, two rounds, medians 120.49 and 118.39 → 120.41 and 117.05 ms,
minimums 117.53 and 116.36 → 115.91 and 114.73 ms. memory.peak over 5 alternating runs: median
40.0 → 39.7 MB, with the same allocations. S 800 → 819 (the first form, with its tables as
`_mm_setr_epi8` calls and tuples that rustfmt splits, cost 35). Gate: 510 pass, 0 fail.

## Step 47: phrase triggers without a branch

After step 46, the line end's token loop still pushed each phrase trigger's position to a `Vec`
behind a test of the slot's `PHRASE` flag: the one branch on what a token is that it kept, and
the `Vec`'s length, loaded and stored at each push, the one value of the loop on the stack.
Step 9 had measured a branch-free push as a loss, when the trigger lists were `Vec`s. Since
step 31 the token loop writes its runs into one slice with cursors, so the positions now go to a
fifth run of `kept`, a store and an add per token like the other four, and the phrase checks
read that run. One line fewer.

Against step 46 (`perf stat -r 5`, interleaved, two rounds): instructions 5,264.2 → 5,305.6 M,
branch misses 12.63 → 11.61 M, cycles 2,719 → 2,706 M in the round without an outlier.

Local, against step 46 (`ab.py`, 41 alternating runs, load average 5 to 7 from other work), four
rounds: medians 120.51, 119.23, 118.19 and 117.84 → 118.32, 118.01, 117.17 and 117.19 ms, minimums
117.50, 116.63, 116.03 and 116.29 → 115.37, 115.46, 115.66 and 115.34 ms. memory.peak over 5
alternating runs: median 40.2 → 37.8 MB, one `Vec` fewer per thread. S 819 → 818. Gate: 510
pass, 0 fail.

## Step 48: the first matches in half the memory

A build that sampled the cgroup's `memory.current` and `memory.stat` every 20 µs from a thread of
its own, with `/proc/self/smaps` at the phase ends, on seed 1 (local medians 39.8 to 40.8 MB, in two
modes 2 MB apart). Memory rises from 16 MB when the plan is ready to 27 MB as the walks start
(per thread 400 KB of query cells, 326 KB of check cells, the window's rows and lists), then
climbs to 39 MB through the walks, and peaks either late in the walks or as the threads group
their first matches, about level. What climbs is `Tally::first`: about 121,800 (query, doc) pairs
per thread, 976 KB each, 11.7 MB on 12 threads, the largest piece of the peak. At the peak the
cgroup held 36.5 to 38.9 MB of anonymous memory, 1.1 MB of kernel memory (0.74 MB of page tables)
and no file pages. Of the anonymous memory, 6 or 8 MB sat on huge pages: the term table's 4 MB
and the checks' 2 MB, as meant, and in the higher mode one more 2 MB page under the term keys
(`TermTable::keys`, 4.9 MB, `calloc`ed, of which only 13 entries, the terms longer than 12
bytes, are ever written). THP is `always` on this box, so the first write into a 2 MiB-aligned
stretch of that block faults in a whole huge page when one is free.

- **The first matches a doc at a time.** The pairs of one doc come together, 3.7 per doc on
  average (32,700 runs for 121,800 pairs per thread), so `first` now holds the query indices
  alone, `u32`, and after each doc's, its id marked with the top bit ([`DOC`]): `Tally::mark`
  at the end of `add_checks`, which runs once per doc, after its phrases; and after each first
  match of a query on sets, whose docs come out of order at the window's end. Grouping reads the
  list from the back, where a doc's id comes before its queries, through the same counting sort,
  which now takes its items as an iterator from the last to the first. 154,500 entries of 4
  bytes per thread instead of 121,800 of 8: 618 instead of 976 KB.
- **The keys off huge pages.** `MADV_NOHUGEPAGE` on the keys' block: 13 pages of 4 KB instead
  of, in most runs, a 2 MB huge page.

Local, against step 47: memory.peak over 5 to 6 alternating runs, three sessions: medians 40.0,
40.85 and 39.8 → 33.8 and 33.7 MB with both changes, 37.9 MB with the keys alone (37.5 to 39.1
against 37.5 to 41.2). `ab.py`, 41 alternating runs, four rounds (the first two on a build identical
but for formatting): medians 116.99, 117.29, 117.51 and 117.49 → 116.33, 116.58, 116.68 and 117.01
ms, minimums 115.09, 115.58, 115.33 and 115.76 → 114.79, 115.16, 115.06 and 115.30 ms. S 818 →
841. Gate: 510 pass, 0 fail. Step 47's memory median, 40.2 → 37.8 MB, was most likely the keys'
huge page coming and going.

## Step 49: memory is at its cap in B, so spend it on time

The board's run of step 48 had memory.peak 37.6 MB against 43.3 MB for step 47, and its points fell
(77.40 → 77.16). The memory factor is `min(M_ref / M, 50)^0.20`, and B's reference used 2,200 MB
(`harness/calibration-seed1.json`), so any peak under 44.0 MB already earns the cap. Across four
board runs (steps 45 to 48, 37.6 to 43.3 MB), K · T^0.5 · S^0.15 is the same, 32.3: the memory
factor did not move. Step 48's 23 lines bought nothing in B.

This step undoes step 48's first-match list (S 841 → 818) and keeps its other part, the two lines
that keep the long terms' `keys` off huge pages, which take about 2 MB off a typical run. It then
spends some of the room under the cap on time: `DENSE_SHARE` 48 instead of 32, so terms held by one
sampled document in 33 to 48 become bitsets. Measured on step 44, that was -1.1% in four rounds
for +0.85 MB.

Full seed 1, step 47 → step 49, `ab.py`, 41 interleaved runs, medians / minimums: 118.95 /
115.45 → 116.45 / 113.25 ms and 118.12 / 115.67 → 115.96 / 113.58 ms (-2.1%, -1.8%).
memory.peak (systemd-run, alternating): 40.2, 37.6, 40.6 → 39.8, 39.6, 39.1 MiB, under the cap
with about 2 MB to spare at the board's usual reading. S 818 → 820. The gate passes (510 of 510).

## Step 50: the probes carry their slots' addresses

After step 49, the line end's token loop, which probes each token's home slot
([`TermTable::find_from`]) and marks the doc's terms, ran 57 instructions per token, 7 of them
stack accesses. For the probe that goes on past its home slot, the loop held the slots' address,
their count's mask and the long keys' address, so it reloaded the addresses of `check_at`, `seen`
and `kept` from the stack at every token, and kept the phrase triggers' cursor there, a load and a
store per token; LLVM also hoisted the slow path's reloads of the slots' address and mask above
the probe's branch.

- **The slot's address in the probe.** [`TermTable::prefetch`] returns the home slot's address
  instead of its index, and the walk stores it beside the key, so the probe loads its slot with
  no shift and no table address. The walk's token loop takes the same number of instructions.
- **Past the home slot out of line.** `TermTable::find_past` takes the probes that go on past a
  home slot, `#[cold]` and never inlined. Only the call needs the table, so the token loop keeps
  every value but two in registers: 52 instructions per token, 3 of them stack accesses (the
  address of `check_at`, and the lists' cursor, loaded and stored), as only the six registers a
  call preserves hold values across it.

`perf stat -r 5`, interleaved, two rounds: instructions 5,191.3 → 4,949.2 M (-4.7%), cycles 2,652
and 2,647 → 2,591 and 2,588 M (-2.2%), branch misses 11.41 → 11.40 M. The address alone: 5,078.3 M
instructions, cycles 2,650 and 2,647 → 2,596 and 2,595 M.

Local, against step 49 (`ab.py`, 41 alternating runs on the P-cores, turbo off), medians /
minimums: seed 1, 113.31 / 111.51 → 111.27 / 109.58 ms and 113.30 / 111.73 → 111.50 / 110.06 ms;
seed 2, 114.00 / 112.49 → 111.92 / 110.26 ms and 114.09 / 112.50 → 111.80 / 110.44 ms (-1.6% to
-2.0%). memory.peak over 5 alternating runs: 38.6 to 39.8 → 38.8 to 39.6 MiB, with the same
allocations. S 820 → 826. Gate: 510 pass, 0 fail.

## Step 51: a short key's tail as it is

After step 50, each probe built the key's `tail` ([`Slot::tail`]) before comparing it with the
slot's: its bytes 8 to 11 and, in the top bit, whether it runs past 12 bytes, four instructions
per token. The probe branches on that same long bit anyway, for the long keys' full compare, and
LLVM had already split `found & long` into a branch on `long` first. On the short side the top
bit is 0, so [`TermTable::holds`] now compares the low half of `key[1]` with the slot's `tail` as
it is, and builds the tail only on the long side. Same answers: a short key's tail is that low
half. The loop takes two instructions fewer per token, as the new layout jumps over the long
side at every token. One line fewer.

Against step 50 (`perf stat -r 5`, interleaved, two rounds): instructions 4,949.7 → 4,875.4 M
(-1.5%), cycles 2,585 and 2,584 → 2,536 and 2,541 M (-1.8%), branch misses 11.40 → 11.38 M.

Local, against step 50 (`ab.py`, 41 alternating runs), medians / minimums: seed 1, 111.29 /
109.86 → 109.85 / 108.60 ms and 111.45 / 110.06 → 110.11 / 108.09 ms; seed 2, 113.15 / 110.19 →
110.80 / 109.11 ms and 111.90 / 110.24 → 110.68 / 109.06 ms (-1.1% to -2.1%). memory.peak over 3
alternating runs: 38.5 to 39.5 → 39.1 to 40.0 MiB, with the same allocations. S 826 → 825. Gate: 510
pass, 0 fail.

## Step 52: the rows' bits set without bounds checks

After step 51, the line end's loop that sets the doc's bit in the row of each of its terms kept
in rows, `bits[store[t]][i / 64] |= 1 << (i % 64)`, took 9 instructions per term, two of them
bounds checks: one on `store[t]` and one on the row. A term id is below `store.len()`, and a term
with the `ROW` flag has a store below the row count, so both reads go unchecked. 1 more line.

Against step 51 (`perf stat -r 5`, interleaved, two rounds): instructions 4,875.4 → 4,832.1 M
(-0.9%), cycles 2,546 and 2,545 → 2,522 and 2,518 M (-1.0%).

Local, against step 51 (`ab.py`, 41 alternating runs), medians / minimums: seed 1, 109.66 /
108.43 → 109.38 / 107.66 ms, and 109.70 / 108.57 → 109.04 / 107.80 ms as committed; seed 2,
110.41 / 108.89 → 109.55 / 108.26 ms; seed 3, as committed, 110.22 / 108.37 → 110.13 / 107.86 ms
(-0.1% to -0.8%). The same allocations. S 825 → 826. Gate: 510 pass, 0 fail.

## Step 53: 37 fewer lines, the same pass

Cuts in code that runs once or once per thread; the hot loops are as they were:

- **The parse.** A thread's share of the lines is a `Queries<Key>`, the same layout as the
  merged queries with each term still its key, so the `Lines` struct goes, and the merge
  offsets each part's `at` instead of summing a per-query length (-8). A malformed line's error
  comes from `ok_or_else` (-1).
- **The plan.** A term's cost is a `u32` that fits on one line (-2); the per-thread plans are
  unzipped in a statement of their own (-1); a BOOL query's check is compiled first and kept
  when no cover term is common (-1); the store and row counts share one `let`, and the row and
  list counts are one field, `sizes` (-3); the term flags are `u32`, and `TermTable::mark`'s
  loop, now one `for_each` without casts, sits where it runs, in `Plan::new` (-5);
  `Plan::phrases` joins the other `impl Plan` (-2).
- **The rest.** One `use` line (-3); `MADV_DONTNEED` named in a comment at its call, like the
  other advice values (-2); `key_hash` inside `TermTable::home`, its only caller (-2); the
  sample makes each new part the sum and adds the old sum into it, with no branch for the first
  part (-3); `intern`'s push on one line, and the sample's and the scan's per-thread locals
  declared together (-4).

Against step 52 (`perf stat -r 5`, interleaved, three rounds, seed 1): instructions 4,829.8 →
4,828.4 M (the plan and the sample), cycles 2,525, 2,480 and 2,523 → 2,488, 2,525 and 2,523 M.
Local, against step 52 (`ab.py`, 41 alternating runs), medians / minimums: seed 1, 109.13 /
107.73 → 108.83 / 107.13 ms; seed 2, 109.46 / 108.24 → 109.47 / 107.76 ms; a second round, seed
1, 109.10 / 107.69 → 109.15 / 107.64 ms; seed 3, 110.02 / 108.60 → 109.79 / 108.02 ms. memory.peak
over five alternating runs: median 39.9 → 39.5 MB. S 826 → 789. Gate: 510 pass, 0 fail.

## Step 54: the rows from the docs' term bits, 64 docs at a time

Of the 36.28 M tokens of seed 1, 28.72 M are query terms, and 11.11 M of those are a doc's first
of a term kept in a bitset row. Each paid a store into the rows run of `kept` and a cursor add in
the token loop, then at the line end a load of its term's store and a read-modify-write of one
bit in a 512-byte row, at random among about 540 rows. The doc's `seen` bits already hold the
same facts, one bit per term id.

So the plan, right after the sample, renumbers the terms: the common ones (543 on seed 1, of
which 532 have a row) take ids 1 up, and a common term in a set query keeps the row of its id.
The others follow in their old order. `hits`, `lens`, the long keys and the queries' terms move
to the new ids at once; the slots take theirs in the pass that marks them, which runs anyway.
A doc's rows are then the first `ceil(rows / 64)` words of `seen` (9 on seed 1). The line end
copies them into `Window::group`, one word per block of 64 rows, and the 64th doc transposes each
64 × 64 block (six vectorized steps) into word `i / 64` of its 64 rows and zeroes it. The pass's
end spills the last, partial 64. A common term in no set query keeps a row nobody reads.

Against step 53 (`perf stat -r 5`, interleaved, two rounds, seed 1): instructions 4,828.4 →
4,676.3 M (-3.1%), cycles 2,523 and 2,523 → 2,424 and 2,426 M (-3.9%), branch misses 11.4 →
11.2 M. The renumbering takes under 1 ms on the main thread before the plan.

Local, against step 53 (`ab.py`, 41 alternating runs), medians / minimums: seed 1, 109.06 /
107.61 → 106.50 / 105.33 ms; seed 2, 110.26 / 108.56 → 107.60 / 105.87 ms (-2.4%).
memory.peak over five alternating runs: median 38.7 → 38.8 MB (the rows round up to a multiple
of 64, and each thread keeps 4.6 KB of blocks). S 789 → 823. Gate: 510 pass, 0 fail.

## Step 55 (reverted): the terms interned while the queries are parsed

Reverted to step 54's code after three board runs: 104.1, 103.9 and 120.4 ms (run-dc45dd0a1279,
run-3dca0206aaaf, run-001fa0eb6f48) against step 54's 101.1 and 100.4 ms, although it was 0.76%
faster locally over nine rounds. The first two board runs came in a spell when D also ran 20%
slow; in the third, A, C and D ran at their usual times. Below is the step as it was measured.

A timeline of step 54 (a build with timestamps, ms since `main`, medians of 12 runs, seed 1,
turbo off, 12 threads on the P-cores). Only the main thread works from 2.3 to 6.0 ms and in the
plan's tail, and only it and one more from 9.9 to 11.9:

| From | To | What | On |
|---|---|---|---|
| 0.12 | 0.82 | 12 parse threads spawned, the last at 0.82 | main |
| 0.25 | 1.68 to 2.29 | the queries parsed, about 1.45 ms a share | 12 threads |
| 2.31 | 3.15 | the term table set up: its huge pages' faults and zeroing 0.68, `MADV_COLLAPSE` 0.17 | main |
| 3.15 | 5.98 | 152,904 term occurrences interned, 18.5 ns each | main |
| 5.98 | 8.01 | the sample: spawned by 6.11 to 6.32, counted in 1.05 to 1.35 ms a thread, 0.22 ms from the last sum to the join | 12 threads |
| 8.01 | 8.87 | the renumbering: the partition 0.25, the id map 0.23, `hits` and `lens` 0.21, the long keys 0.06, the queries' terms 0.08 | main |
| 8.94 | 9.86 | the queries planned, 0.6 ms a share, joined 0.13 ms after the last | 12 threads |
| 9.86 | 11.93 | the plan's tail: the checks' block 0.47 and grouping 0.88 on their thread; the phrases 0.6 (the spawn included), the flags 0.32, the stores 0.29 and marking the slots 0.84 on main | 2 threads |
| 11.93 | 12.21 | the plan's lists freed | main |
| 12.31 | 13.14 to 13.68 | 12 scanning threads spawned by 12.51, each 0.8 to 1.2 ms setting up (`Tally::new` 0.6 to 0.9) | 12 threads |

The largest serial piece is the intern, and the table it needs took 0.84 ms more on the main
thread while the other threads had nothing to do. The table's size needs only the term count,
and a term follows every space but those before an operator, the only capitals after a space,
exactly for every line that parses (checked on all 510 gate inputs). So 11 threads now parse,
while the main thread counts the terms (one pass over pairs of bytes summed in `u32`, 0.2 ms)
and sets up the table; it then joins the shares in order and interns each as soon as it is
parsed, so the ids stay in file order. A line that does not parse stops the parse, as before;
an invalid line counts zero or more terms, so the table holds every term of the shares that
parse. In the same build, interleaved with step 54 (12 runs each): the table is ready at 2.07 ms
instead of 3.02, the intern ends at 5.12 instead of 5.87, the plan at 10.94 instead of 11.89,
the walks start at 12.00 to 12.44 instead of 13.09 to 13.80, and the output is written at 102.60
instead of 103.62 ms.

Local, against step 54 (`ab.py`, 41 alternating runs), medians / minimums, nine rounds: seed 1,
106.51 / 104.86 → 105.41 / 104.33 ms; seed 2, 106.85 / 105.29 → 106.28 / 104.70 ms; seed 3,
106.73 / 105.75 → 106.05 / 104.50 ms; seed 1, 106.63 / 105.23 → 105.57 / 104.24 ms; seed 1,
106.50 / 105.09 → 106.02 / 104.34 ms; seed 3, 106.94 / 105.02 → 106.02 / 104.86 ms; seed 2,
106.64 / 105.09 → 105.74 / 103.96 ms; seed 4, 107.80 / 106.00 → 106.58 / 105.30 ms; seed 1,
106.15 / 105.17 → 105.77 / 104.41 ms: -0.36% to -1.13%, -0.76% on average, so a trial.
memory.peak over five alternating runs: median 39.2 → 39.0 MB. S 823 → 829. Gate: 510 pass,
0 fail.

## Step 56: the probe without a branch on a key's length, and the triggers' loop unchecked

Where the pass goes, measured on step 55's code, whose pass is step 54's (`perf record` of cycles,
instructions and branch misses on the `profiling` build, seed 1, turbo off; `perf stat`: 4,678.8
M instructions, 2,424 M cycles and
11.13 M mispredicted branches, 129 instructions and 67 cycles per token). Counted by a build
with counters: 36.28 M tokens, 28.72 M of them query terms, 22.24 M a doc's first of a term,
7.18 M BOOL triggers, 1.75 M first sights of a listed term, 0.92 M phrase triggers, 12.94 M
checks, and 1.37 M probes past the home slot (`find_past`).

| Stage | Cycles | Instructions | Mispredicts |
|---|---|---|---|
| `Tokens::line`: token ends, keys, hashes, prefetches | 26.9% | 30.8%, 39.7 a token | 37% |
| The line end's token loop: probes, seen bits, runs | 26.7% | 31.7%, 40.9 a token | 1% |
| `find_past` | 3.8% | 1.4% | 17.6% |
| The BOOL triggers' loop | 5.4% | 5.9%, 38 a trigger | 3.7% |
| `add_checks` | 11.6% | 7.5%, 27 a check | 3.5% |
| `seen` cleared, first matches | 4.6% | 3.6% | 10.2% |
| The lists' pushes and the phrases | 5.4% | 1.9% | 3.6% |
| `flush` and `spill` | 6.4% | 4.6% | 7% |

The two token loops run at about 2.2 instructions per cycle on each of a core's two threads, so
what they cost is mostly their instructions. In `Tokens::line`, most mispredicts are the exit
of the per-block loop over a block's 9.1 token ends on average, about once per block.

- **The probe's first test.** [`TermTable::find_from`] compared a key's bytes 8 to 11 with the
  slot's `tail` and branched first on whether the key runs past 12 bytes, for which it built
  the long bit and compared the whole key: a `bt`, a branch and a jump over the long side at
  every token. It now XORs the key's whole second word with the `tail`, zero-extended. A key
  past 12 bytes has bytes 12 to 15 that are not zero and never matches there, nor does a term
  past 12 bytes, whose `tail` has its top bit set. [`TermTable::intern`] gives such a term's
  home slot `MORE`, and `find_past` now starts at the home slot itself, with the full compare
  ([`TermTable::holds`], as before). Seed 1 has 13 such terms, and 0.19% of its tokens run past
  12 bytes. Instructions 4,474.6 → 4,294.4 M, cycles 2,416, 2,386 and 2,418 → 2,354, 2,351 and
  2,356 M, branch misses the same (11.2 M), over three interleaved rounds of `perf stat -r 5`.
  The same with the line at 12 or more bytes, which kept a key of exactly 12 bytes off the
  first test too: 4,333.6 M instructions but 12.6 M branch misses and the same cycles, as seed
  1 has 3,809 terms of exactly 12 bytes (of 75,747).
- **The triggers' loop.** It loaded `check_at[t]` and `check_at[t + 1]` with a bounds check
  each, wrote its four indices to `todo` behind two more, and took the range's length through
  two conditional moves, about 38 instructions per trigger with four values reloaded from the
  stack. The two entries are now one unchecked `[u32; 2]` load, the four indices one store, and
  the cursor moves on by the range's length, which a loop past the first four fills: the
  ranges of a doc's triggers are disjoint, so `todo`'s 4 spare entries cover the store.
  Instructions 4,678.8 → 4,474.6 M (-4.4%), cycles 2,398, 2,423 and 2,425 → 2,392, 2,390 and
  2,417 M: the loop waits on its loads of `check_at` more than it runs.

Both parts were built and measured on step 55's code first, then moved onto step 54's when step
55 was reverted; neither touches what step 55 changed. Against step 54 (`perf stat -r 5`,
interleaved, three rounds): instructions 4,676.2 → 4,291.4 M (-8.2%), cycles 2,397, 2,425 and
2,427 → 2,361, 2,358 and 2,359 M (-2.3%), branch misses 11.2 M both. `ab.py`, 41 alternating
runs, medians / minimums: seed 1, 106.62 / 105.06 → 104.17 / 102.96 ms; seed 2, 106.85 / 105.30
→ 105.47 / 103.73 ms and 106.92 / 105.63 → 104.94 / 103.49 ms; seed 3, 107.14 / 105.17 → 105.31 /
103.38 ms; seed 1, 106.40 / 104.74 → 104.25 / 103.08 ms; seed 3, 107.07 / 105.20 → 105.09 / 103.92
ms (-1.3% to -2.3%, -1.8% on average, so a trial). On step 55's code, against step 55: seed 1,
105.90 / 103.26 → 103.11 / 101.92 ms; seed 2, 106.40 / 104.50 → 103.71 / 102.82 ms; seed 3,
106.56 / 105.30 → 104.13 / 103.04 ms; the triggers' loop alone, seed 1, 105.67 / 104.56 → 105.24 /
104.21 ms, seed 2, 106.48 / 105.01 → 106.05 / 104.70 ms. memory.peak over five alternating runs:
38.2 to 39.9 → 38.4 to 40.2 MB. S 823 → 821. Gate: 510 pass, 0 fail.

## Step 57: the id lists from one log, sorted at the flush

After step 56, a build that skipped the id lists' pushes (wrong answers, to size the stage) ran
137 M fewer cycles (-5.9%, the flush's lists included), and `perf record` of L2 misses put 27%
of the pass's 6.6 M on the pushes: about one per push. Seed 1 has 1.75 M of them, a doc's first
sight of a listed term, about 7 per document, 28.7 K per window. Each loaded the term's store,
then one of 3,452 `Vec` headers, then wrote the end of that list: the ends of thousands of lists
lie at random, and few stay in L2 between two pushes.

Now a document's entries go to the end of one log, list `j` holding document `start + i` as
the `u32` `j << 5 | i % 32`, and the log records where each 32 documents' entries end. At the
flush, a counting sort moves them into the lists: a count per list, the sums, and a scatter in
log order, so that each list stays ascending; the lists are then `ids[at[j]..at[j + 1]]`. The
log, 115 KB a window on seed 1, is written in order, and the sort's 13.8 KB of counts stay in
L1. An entry of a `u64`, `j << 16 | i`, needs no ends, but its log took 2.5 MB more at the
peak (memory.peak 41.2 to 41.3 MB) for the same cycles.

Against step 56, measured after the reboot of Nov 1 with turbo off: `perf stat -r 5`,
interleaved, instructions 4,291.4 → 4,318.8 M (+0.6%), cycles 2,364 and 2,365 → 2,301 and 2,302
M (-2.7%), branch misses 11.2 → 11.0 M. `ab.py`, 61 alternating runs, medians / minimums: seed
1, 103.47 / 102.07 → 101.56 / 99.99 ms; seed 2, 103.31 / 102.04 → 100.98 / 99.90 ms; seed 3,
103.56 / 101.78 → 100.92 / 99.69 ms (-1.8% to -2.5%). The same build before `fmt` merged two
lines: seed 1, 103.49 / 102.11 → 101.20 / 99.56 ms; seed 2, 104.00 / 101.76 → 101.33 / 99.89
ms; seed 3, 103.70 / 101.90 → 100.90 / 99.78 ms (-2.2% to -2.7%). Against step 54, seed 1:
105.58 / 104.06 → 101.47 / 99.69 ms (-3.9%). memory.peak over five alternating runs: 38.6 to 40.1
→ 38.2 to 40.0 MB. S 821 → 842. Gate: 510 pass, 0 fail.

## Step 58: the most frequent term of each home slot moved into it

A probe that finds its term at the home slot stops there; any other goes on in `find_past`, out
of line, with a mispredicted branch at its loop's end (3.9% of the cycles). On seed 1 that was
1.46 M probes (the sample's included), 1.17 M of them for a term that sits further on: the
terms land in file order, so the first of those with the same home takes it, however rare.

[`TermTable::settle`] now visits each home slot with `MORE` before the slots are marked, and
moves into it, of the terms of 12 bytes or less whose home it is, the one with the most sample
hits: terms with the same home lie in one run of slots from it, as does a term in a home slot
not its own, which can move on to any later slot of the run, so either pair can trade places.
`MORE` stays with the slot. Probes past the home slot: 1.46 → 0.82 M, of which 0.53 M find a
term (the less frequent of two that share a home). Taking only terms with the same home as the
one there, and no term out of a home not its own: 0.84 M, about the same cycles.

On the main thread after the marking, the pass took 0.75 ms, and the wall gained 0.8% to 0.9%
(seeds 1 to 3, 100.38 / 99.23, 100.50 / 99.16 and 100.26 / 97.78 against 101.33 / 99.50, 101.34 /
99.97 and 101.21 / 99.68 ms). It now runs on a thread of its own while the main thread groups
the phrases, flags the terms and numbers the stores (1.2 ms), on the old ids, whose sample hits
it reads through the renumbering.

Against step 57: `perf stat -r 5`, interleaved, instructions 4,318.8 → 4,279.9 M, cycles 2,304
and 2,300 → 2,256 and 2,256 M (-2.1%), branch misses 11.0 → 10.1 M. `ab.py`, 61 alternating
runs, medians / minimums: seed 1, 101.82 / 100.08 → 100.20 / 98.82 ms; seed 2, 102.41 / 100.24 →
100.31 / 97.84 ms; seed 3, 100.86 / 99.44 → 99.67 / 98.25 ms; seed 1, 101.26 / 99.77 → 99.72 /
98.34 ms (-1.2% to -2.1%, -1.6% on average, so a trial). The same change before its lines were
tightened: seed 1, 101.50 / 100.32 → 99.71 / 98.29 ms; seed 2, 101.09 / 99.98 → 99.52 / 97.59
ms; seed 3, 101.23 / 99.53 → 99.72 / 98.06 ms. memory.peak over five alternating runs: 39.2 to 39.5
→ 39.0 to 39.7 MB. S 842 → 864. Gate: 510 pass, 0 fail.

## Step 59: the probe's first test as one select, and no prefetch of `check_at`

`find_from` went on to `find_past` unless `found || !more`, two `sete`, an `or` and a `test`
behind an empty `asm` block that kept LLVM from making two branches of it, and the flags' select
then tested `found` once more. Now `on` is the key's difference when the slot has `MORE` and 0
otherwise, one `cmov`, and the branch tests it: two instructions fewer per token in both token
loops. Alone, against step 58: instructions 4,279.9 → 4,243.1 M, cycles 2,256 to 2,262 → 2,202 to 2,232
M; `ab.py`, 61 alternating runs, seed 1, 99.17 / 98.15 → 98.37 / 97.24 ms; seed 2, 99.13 / 98.16
→ 98.53 / 97.17 ms (-0.6% to -0.8%).

The token loop at the line end also prefetched each token's `check_at` entry for the BOOL
triggers' loop (step 44), and with every register taken it reloaded `check_at`'s address from
the stack for it, per token. Without the prefetch: 65 M fewer instructions (1.8 per token), and
the triggers' loop now waits on loads of `check_at` that are mostly L2 hits; cycles 2,216 to 2,235
→ 2,193 to 2,221 M on top of the select. After step 56 the prefetch's removal alone was within the
noise (below).

Against step 58: `perf stat -r 5`, interleaved, instructions 4,279.9 → 4,178.0 M, cycles 2,255
and 2,255 → 2,220 and 2,217 M (-1.6%), branch misses 10.1 M both. `ab.py`, 61 alternating runs,
medians / minimums: seed 1, 99.64 / 98.41 → 98.34 / 97.12 ms; seed 2, 99.63 / 98.42 → 98.14 /
96.67 ms; seed 3, 99.19 / 98.14 → 97.71 / 96.19 ms; seed 1, 99.91 / 98.40 → 98.06 / 96.77 ms;
seed 2, 99.75 / 98.32 → 98.23 / 97.05 ms (-1.3% to -1.9%, -1.5% on average, so a trial). memory.peak over five alternating runs: 38.8 to 39.7 → 38.3 to 39.4 MB. S 864 → 863.
Gate: 510 pass, 0 fail.

## Step 60: the corpus faulted in during the intern, and the chunks freed during the write

A timeline of step 58 (step 59 changed only the token loop), in a build with timestamps as in
step 55's section (ms since `main`, medians of 12 runs, turbo off), and of this step in the same
build, interleaved with it:

| What | Step 58, seed 1 | Step 60, seed 1 | Step 58, seed 2 | Step 60, seed 2 |
|---|---|---|---|---|
| the queries parsed | 2.15 | 2.03 | 2.07 | 2.17 |
| the term table set up (main only) | 3.01 | 2.92 | 2.97 | 3.10 |
| the terms interned (main only) | 5.95 | 5.76 | 5.86 | 5.95 |
| the sample joined | 7.62 | 7.48 | 7.79 | 7.73 |
| renumbered (main only) | 8.45 | 8.34 | 8.57 | 8.49 |
| planned | 9.37 | 9.30 | 9.68 | 9.41 |
| the plan's tail ends (3 threads) | 11.48 | 11.22 | 11.64 | 11.37 |
| the walks start | 12.76 to 13.24 | 12.42 to 12.83 | 12.97 to 13.56 | 12.59 to 13.00 |
| a walk takes | 75.15 to 76.01 | 74.01 to 75.32 | 74.91 to 76.01 | 74.01 to 74.67 |
| the walks end | 87.78 to 89.99 | 86.34 to 89.19 | 87.73 to 89.85 | 86.50 to 88.82 |
| the scanning threads' tails end | 91.85 to 93.86 | 90.44 to 92.68 | 92.19 to 93.67 | 90.49 to 92.29 |
| formatted | 95.22 | 94.09 | 95.10 | 93.62 |
| `exit` called | 95.81 | 94.76 | 95.75 | 94.26 |
| the process gone | 97.28 | 95.77 | 97.39 | 95.14 |

From the parse to the walks, only the main thread works for 3.8 ms (the table and the intern)
and 0.9 ms (the renumbering), and three threads for 1.9 ms (the plan's tail). At the end, the
walks end 2.1 to 2.6 ms apart (medians of the spread), and every scanning thread's tail takes
3.9 to 4.8 ms: the last one's, on step 58, is the corpus pages dropped 0.72, the flush 0.53, the
window freed 0.92, the merge 0.33 and its first matches grouped 1.61 (placing them 1.08). The
exit takes 1.55 to 1.65 ms, against 0.13 for a process that exits at once.

The walk's first touch of each 64 KiB of the mapped corpus is a page fault even when the file
is cached: the fault maps the 16 pages around it. Seed 1 draws about 3,900 of them, some 2.7 µs
each, in the middle of the walks. So, as soon as the queries are parsed, `answer_all` starts
`threads / 4` threads (3 of 12) that read the corpus a byte per 64 KiB, each a third of it, and
are never joined. They run from 2.06 to 2.09 to 5.45 to 5.54 ms (seed 1), inside the intern, which
does not slow. With more of them, one shares the main thread's core and the intern slows. A
walk takes 1.0 ms less. The faults are as many (17,320 → 17,337 a run, `perf stat -r 10`, two
rounds), but kernel cycles go 159.0 and 158.4 → 145.7 and 145.3 M. After step 38, helper
threads reading a byte per 4 KiB page from the start, at nice 19, slowed the parse, the sample
and the plan by about 2 ms (its entry in the list below); these read 16 times fewer bytes, and
only while one thread works.

The chunks, which `answer_all` never freed (step 38), are now freed on a detached thread once
they are formatted, while `main` writes the output: the exit then has less to unmap, 1.65 →
1.10 ms (seed 1) and 1.55 → 0.94 ms (seed 2). Freed on the main thread, they took 0.84 ms and
saved 0.91 ms of the exit.

Local, against step 59 (`ab.py`, 61 alternating runs), medians / minimums: seed 1, 98.18 / 96.97
→ 96.73 / 95.55 ms; seed 2, 98.08 / 96.44 → 96.60 / 95.22 ms; seed 1, 97.88 / 96.53 → 96.78 /
95.36 ms; seed 2, 97.94 / 96.96 → 96.49 / 94.54 ms (-1.1% to -1.5%). Before, against step 58:
seed 1, 99.84 / 98.22 → 97.97 / 96.89; seed 2, 99.34 / 98.51 → 98.18 / 97.14; seed 1, 101.63 /
98.33 → 99.68 / 96.72; seed 2, 99.58 / 98.19 → 98.41 / 96.73 (-1.2% to -1.9%). First measured
on step 54, against it: seed 1, 105.58 / 104.39 → 104.06 / 102.57; seed 2, 106.14 / 104.31 →
104.66 / 102.39; seed 1, 105.61 / 104.12 → 103.88 / 102.59; seed 2, 105.12 / 103.95 → 103.46 /
102.16 (-1.4% to -1.6%); the prefaulting alone, three rounds: seed 1, -0.95%, -1.39%, -0.95%;
seed 2, -0.78%, -0.34%, -1.31%. memory.peak over five alternating runs, seed 1: 38.8 to 39.4 →
39.4 to 40.2 MB, median 39.3 → 39.7. S 863 → 868. Gate: 510 pass, 0 fail.

## Step 61: a zero in a register for the line end's selects, and four instructions fewer per token in the tokenizer

The token loop at the line end takes three `cmov`s against 0 per token, and LLVM set a register
to 0 before each (`mov $0`), as none was free to hold one across the loop. A zero made once per
line behind an empty `asm` block, which the optimizer cannot see through, now stays in a
register for all three: 43 M fewer instructions. Alone, on step 59: cycles 2,216 to 2,226 → 2,204
M (-0.8%); `ab.py`, 61 alternating runs, seed 1, 98.13 / 96.86 → 97.52 / 96.06 ms; seed 2,
97.97 / 96.68 → 97.41 / 96.03 ms; seed 3, 97.96 / 96.56 → 97.15 / 95.89 ms (-0.6% to -0.8%),
under the bar alone. On step 60: 4,178.7 → 4,135.8 M instructions, cycles 2,225 and 2,221 →
2,213 and 2,205 M.

`Tokens::line` took 28 µops per token, three of them for a token past 32 bytes, which is rare.
Before the length test, the loop set a register to the key of 0xFF bytes (`vpcmpeqd`) for every
token, so that the test could jump straight to where both paths meet; and the mask's address
took a `mov` of the table's address and a `sub` of the length. The length test is now marked
cold (`std::hint::cold_path`), so that the 0xFF key is set on its own path only, and `fold`
takes the length negated, `start - end`, so that the mask's address is that plus a constant,
folded into the `vpand`: 74 M fewer instructions, 2 per token. Then the token's start and end,
each `base` plus a bit's position, are offsets from the block instead: the start of a token
open since an earlier block lies below 0, wrapped, and moves back 64 per block, and the test
that a token's 32 bytes lie in the chunk compares, signed, against a bound fixed within a block.
25 M fewer instructions; short of registers, LLVM keeps the block's address on the stack and
loads it per token.

`perf stat -r 5`, interleaved, seed 1, instructions and cycles (M):

| Build | instructions | cycles |
|---|---|---|
| step 60 | 4,178.7 | 2,225, 2,221 |
| the key's length test and mask only | 4,104.5 | 2,198, 2,204 |
| the zero only | 4,135.8 | 2,213, 2,205 |
| both | 4,061.6 | 2,178, 2,185 |
| step 60, again | 4,178.7 | 2,222, 2,225 |
| both, and the offsets (this step) | 4,037.1 | 2,159, 2,161 |

On seed 2: 4,179.6 → 4,038.1 M instructions, cycles 2,189 → 2,161 M. Branch misses 10.1 M in
all.

`ab.py`, 61 alternating runs, medians / minimums, step 60 → this step: seed 2, 97.69 / 96.00 →
95.58 / 93.42 ms; seed 1, 96.64 / 94.94 → 94.65 / 93.10 ms; seed 3, 96.32 / 94.81 → 94.22 /
92.51 ms; the same code before its comments: seed 1, 96.55 / 94.96 → 94.70 / 92.76 ms; seed 3,
96.27 / 94.86 → 93.72 / 92.56 ms (-1.9% to -2.6%, -2.2% on average). Without the offsets: seed
1, 96.30 / 95.11 → 94.88 / 93.36 ms; seed 2, 96.71 / 95.08 → 95.05 / 93.37 ms; seed 3, 96.01 /
94.68 → 94.90 / 93.50 ms; seed 2, 96.52 / 95.26 → 94.96 / 93.22 ms (-1.2% to -1.7%).
memory.peak over five alternating runs, seed 1: 39.2 to 39.7 → 39.1 to 40.1 MB, median 39.5 both.
S 868 → 877. Gate: 510 pass, 0 fail.

On the board (run-280052ffeb2c) this step drew 97.9 ms against step 60's 95.2 ms, but the same run was slow on
every track: D 123.1 ms against 118.4 and A 16.9 ms against 16.65, with C unchanged. A 2% step is below what one
board run resolves, so it stays on the local measurements above.

## Step 62 (reverted): a doc's runs packed 8 tokens at a time, and the home slot from the key's first word

Reverted to step 61's code after two board runs: 105.4 and 106.3 ms (run-e89cc599f8ef,
run-63da3c47097b) against step 61's 97.9 and 95.1 ms, although it was 2.7% faster locally over
five rounds, measured on battery power. In the first run A, C and D ran within 2% of their usual
times; in the second, A ran 42% and C 23% slow. Step 61's code, back on the board, ran 98.1 ms
(run-35a813ada612) in a run where A and C ran 4% and 7% slow, so step 62 cost about 7% there. Below is
the step as it was measured.

`perf record` of step 61 (seed 1, turbo off): the line end's token loop is 37 instructions per
token and 24% of all cycles, at 2.6 instructions per cycle on each of a core's two threads, so
its cost is its instructions. Twelve of the 37 kept the four runs of `kept`: four stores, the
three cursors' tests and adds, and the select of the flags of a term new in the doc.

The loop now stores one value per token: the slot's `flags`, the term's id times 32 and its
flags, with `LISTED` and `BOOL` dropped by the select when the doc already held the term. Then
[`pack`] takes the doc's tokens 8 at a time: the 8 flags shifted down make the ids, and for each
run the flag shifted to each lane's top bit makes an 8-bit mask, whose entry in a 256-entry
table (2 KB, built at compile time) is the permutation that moves the marked lanes to the front;
one store writes them at the run's cursor, which moves on by the mask's popcount. The lists and
BOOL runs take the ids, the phrase run the positions. A store writes 8 entries, so each run starts
8 past the room of the last, and the 8 entries past the doc's tokens are zeroed, as the last load
reads them. The token loop is 27 instructions, the packing 34 per 8 tokens, with no branch but
the loop's exit, once per doc (+0.28 M mispredicts). The table's permutation built per mask by
`pdep` and `pext` instead took about 7 instructions more per run and 8 tokens: 4,037.1 → 3,986.0 M
instructions, cycles the same as step 61 (2,137 and 2,142 → 2,139 and 2,139 M).

[`TermTable::home`] hashed the exclusive or of the key's first two words, a `vpshufd` and a
`vpxor` before the `vmovq` in the tokenizer, per token. It now multiplies the first word alone:
terms whose first 8 bytes agree share a home slot, and [`TermTable::settle`] moves the most
frequent into it. Mispredicts +0.16 M, from the probes past it.

`perf stat -r 5`, interleaved, instructions, cycles and branch misses (M):

| Build | instructions | cycles | branch misses |
|---|---|---|---|
| step 61 | 4,037.1 | 2,118, 2,132 | 10.12 |
| the packing only | 3,892.7 | 2,106, 2,109 | 10.41 |
| the packing only | 3,892.7 | 2,111, 2,111 | 10.40 |
| the packing and the hash | 3,823.2 | 2,096, 2,090 | 10.56 |
| step 61, seeds 1, 2, 3 (cycles: means of two) | 4,037.1, 4,038.1, 4,040.4 | 2,161, 2,159, 2,153 | 10.11 |
| this step, seeds 1, 2, 3 (cycles: means of two) | 3,823.2, 3,823.1, 3,825.9 | 2,073, 2,079, 2,084 | 10.56 |

`ab.py`, 31 alternating runs, medians / minimums, ms. The packing only, against step 61: seed 1,
94.21 / 92.92 → 92.09 / 90.75; seed 2, 94.42 / 93.15 → 92.85 / 91.68; seed 1, 94.60 / 93.26 →
92.98 / 91.01; seed 2, 94.80 / 93.58 → 92.69 / 91.18; seed 3, 93.84 / 92.91 → 91.91 / 90.30; seed
1, 94.68 / 93.19 → 92.43 / 91.78; seed 1, 96.86 / 94.50 → 95.12 / 92.60; seed 3, 95.95 / 94.23 →
94.09 / 92.68 (-1.6% to -2.4%, -2.0% on average). Three-way, step 61, the
packing only and this step: seed 1, 94.88 / 93.27, 92.74 / 91.31 and 91.91 / 90.91; seed 3, 94.32
/ 93.07, 92.61 / 91.09 and 91.85 / 90.05. This step against step 61: seed 2, 94.63 / 92.91 →
91.94 / 90.75; seed 1, 94.45 / 93.18 → 92.55 / 91.01; seed 3, 94.46 / 93.11 → 91.82 / 90.49 (-2.0%
to -3.1% over five rounds, -2.7% on average). memory.peak over five alternating runs, seed 1:
39.1 to 39.5 → 39.5 to 40.0 MB, median 39.5 → 39.6. S 877 → 901. Gate: 510 pass, 0 fail.

## Step 63 (trial): step 62's packing alone

Step 62 put two changes on the board at once and ran about 7% slower there than step 61. The
packing is computation on the line end's own data; the hash of the key's first word alone puts
terms that share their first 8 bytes on one home slot, so more probes go past it into the term
table. This step keeps the packing and step 61's hash, to tell on the board which of the two cost
the time. Locally, on battery, the packing alone measured -1.6% to -2.4% against step 61 over eight
`ab.py` rounds (step 62's section). Gate: 510 pass, 0 fail. S 877 → 901 (static_axes).

## Step 66: step 64's parallel intern again

Step 64's one board run, run-bb4ed7d47695 (93.4 ms), came in a run where A, C and D read 1.5 to 3.4%
above their best, so it did not show whether the change pays on the box. Re-measured today on
step 63's code (local, turbo off, `ab.py`, 41 interleaved runs): seed 1 median 94.76 → 92.01 ms
and, in a second session, 95.05 → 92.26 ms; seed 3 93.94 → 91.22 ms, -2.9%. Gate: 524 pass, 0
fail, now with unseen full seeds 51 to 54. S 901 → 961 (`static_axes`). The first push meant to carry
it, 9c3eb3c, lost the source file to a reset of the working tree before the commit and carried
step 65's code instead, so this is its first board trial since step 64. It was to be kept only if its
board runs beat step 63's 90.6, 91.1 and 92.9 ms. Its run, run-33959bbbe488, read 87.3 ms, in a
run where A and C read 3.1% and 0.5% above their best: 3.6% below step 63's best run. Kept.

## Steps 67 to 69: 14 fewer lines, same code paths

The last afternoon, no time left for speed work, so I went after lines that cost points and do
nothing for speed. All three changes sit outside the document walk (the plan, the grouping and
the start-up intern), each passed the 524-case gate, and S went 961 → 957 → 953 → 947
(`static_axes`). Each is worth about 0.02 points at the same T, far below what one board run
resolves, so they share the rows above.

- Step 67: `group_into` counts and prefix-sums with `for_each` on one line each, instead of two
  three-line `for` loops. Local, turbo off, `ab.py`, 41 interleaved runs on seed 1: median
  96.37 → 93.00 ms (noise; the minimums are 90.52 and 90.18 ms).
- Step 68: the eight-line table of the six check masks became `has(k)`, three lines. Mask `k` is
  `(!0 / (2^(2^k) + 1)) << 2^k`: the division repeats a group of `2^k` ones in every `2^(k+1)`
  bits, and the shift puts the zeros first. A standalone check compared all six against the old
  table before the swap. Local, turbo off, 41 runs, two rounds with the order swapped: medians
  96.15 → 107.75 ms and 94.87 → 93.77 ms, means 104.06 → 105.32 and 101.62 → 101.84 ms, so the
  first round's median is B's two-mode noise, not the change. The masks are only built while
  planning.
- Step 69: in the intern, the claim of an empty slot is
  `let (Ok(f) | Err(f)) = head.compare_exchange(...)` (both arms hold the slot's previous head,
  0 when this thread won it), in place of a four-line `.err().unwrap_or(0)` chain, and the
  length and long-key stores are two one-line statements instead of a six-line block. Local,
  turbo off, 41 runs: median 90.68 → 90.44 ms.

## Experiments that did not stay

- **After step 63, 32-byte aligned checks, a branch-free `add_checks` and immediate vector
  shifts in `pack`** (pushed as 052ab6e). Local, turbo off, `ab.py`, 41 interleaved runs, seed 1:
  median 95.05 → 94.54 ms (-0.5%, inside the runs' spread). The board counted S 901 → 956, about
  0.19 points, more than the time could return. Its two board runs (step 65): run-e40ad8747964,
  94.4 ms, in a run where A, C and D read 14 to 43% above their best, and run-4d543d6cf37e (pushed as
  9c3eb3c), 95.6 ms, where C and D read 1.6% and 3.4% above theirs. Not kept: B went back to step
  63's code.
- **After step 63, where the time goes, measured for three redesigns** (local, turbo off, on AC,
  seed 1, step 63's binary). The question was whether a larger change could make B 10% faster
  locally, the least a board trial would need to show through the box's noise (steps 62 and 64).
  - `perf stat -r 5`: 104.2 ms ±5.7%, 4,111 M instructions, 2,355 M cycles (1.75 per cycle),
    10.8 M branch misses, 9.09 M loads that missed L2 (0.25 per token of the 36.3 M), 0.38 M
    that missed L3, 0.33 M page walks. Top-down: retiring 52.9%, bad speculation 14.7 (branch
    mispredicts 13.2), back end 19.3 (memory 8.9, core 10.4), front end 13.1.
  - Sampled on the event itself: 87% of the L2 misses fall in the scanning loop's line end. Of
    the branch misses, 36% are one branch, the tokenizer's `while line != 0`: it exits once per
    block, after as many ends as the block's text holds, so no history predicts it. The probe's
    two tests (`find_from`, `position`) take 15%; the rest spread over the line end's loops,
    none above 3%. Cycles by source line: none above 5.8%.
  - A timeline from 5 kHz samples (the `profiling` build): the scan starts at 15.3 ms. The first
    16 ms hold 603 samples where 12 busy CPUs would give 960, idle time worth about 6 ms of the
    scan on all 12 CPUs, which then runs to 93 ms.
  - The three redesigns, each against the share it could remove: a term table small enough for
    L2, at most the 8.9% memory share; a line end batched by the kind of term, already the
    branch-free token loop of steps 41-47, its remaining misses loop exits whose counts follow
    the text, about 5% at most; the scan started during the plan, at most the 6 ms idle.
    Step 64's parallel intern is the largest piece of the last one, re-measured today with
    `ab.py`, 41 interleaved runs: seed 1 median 94.76 → 92.01 ms, seed 3 93.94 → 91.22 ms, -2.9%.
    No combination came near 10%, so none was built.

- **After step 62, the tokenizer's hash constant** (seed 1, `perf stat -r 5`, interleaved, two
  rounds). The multiply's 64-bit constant takes a register, so the tokenizer reloads its block's
  address from the stack per token. Freeing it:
  - A constant that fits a sign-extended 32-bit immediate, `0xFFFF_FFFF_9E37_79B1`, so that
    `imul` takes it inline: the block's address then stays in a register, but a short key's
    product is small, and negated it has its top bits all set, so short keys pile into a few home
    slots: 3,823.2 → 40,959.0 M instructions, branch misses 10.6 → 31.4 M.
  - `_mm_crc32_u64(0, key[0])`, the hash's top bits shifted down from 32: the block's address
    stays in a register, but a `xor` zeroes the accumulator per token. Instructions 3,823.2 →
    3,813.5 M, cycles 2,089 and 2,088 → 2,084 and 2,087 M, branch misses 10.57 → 10.50 M.
  - `_mm_crc32_u64(key[0], key[0])`, the key as its own accumulator: one instruction, but a linear
    map of too low a rank, 192,796 M instructions and 46.6 M branch misses.
- **After step 62, the probes 32-byte aligned.** The tokenizer's 32-byte store of a key takes the
  most cycle samples of its loop, and at a 40-byte stride 3 in 8 of them cross a cache line. As a
  `#[repr(align(32))]` struct of 64 bytes, none does: instructions 3,823.2 → 3,827.6 M, cycles
  2,095 and 2,093 → 2,116 and 2,111 M (+0.9%), the larger buffer costing more than the splits.
- **After step 62, the time outside the walks** (local, turbo off, on battery; timelines from a
  build with timestamps on step 61's code, seed 1, medians of 12 runs, ms since `main`; `ab.py`
  on step 62's code, 25 or 31 alternating runs, medians / minimums, ms):
  - The timeline: parsed 2.17, table set up 3.04, interned 5.86, sample joined 7.53, renumbered
    8.39, planned 9.31, the plan's tail ends 11.21, the scanning threads start setting up at
    11.74 to 12.06 and walk from 12.48 to 13.11, the walks end at 84.48 to 87.67, the tails at
    88.55 to 90.71, formatted at 92.05, written at 92.65; `ab.py`'s wall 95.77. At the exit 21.7 MB
    is still mapped (18.5 MB anonymous), with 688 KB of page tables and 89 mappings.
  - Ranked by walk end, the first thread to end its walk took 4.9 ms over its tail and the last
    3.4 ms: the walks end over 2.8 ms (84.10 to 86.93) and the tails over 1.4 ms (88.99 to 90.36). The
    last thread's tail: corpus pages dropped 0.89, last flush 0.43, window freed 0.08, merge
    0.22, first matches grouped 1.41 (allocations 0.13, counting 0.21, placing 1.00, the list
    freed 0.08). The format threads start 0.08 to 0.24 ms after the join and take 0.93 ms each.
    Over 12 runs each of the 12 chunks was the last to end its walk in some run, so no static
    split of the corpus evens the walks out.
  - The last walk to end leaving its corpus pages to the exit (a counter passed to `scan`), 4
    more lines: seed 1, 92.79 / 90.67 → 92.61 / 90.78 and 92.03 / 91.27 → 91.95 / 90.94; seed
    2, 92.50 / 91.29 → 93.19 / 91.13 and 92.38 / 90.97 → 92.71 / 91.40, within the noise. The
    drop moved out of `scan`, after the grouping, the last one again skipped: seed 1, 95.30 /
    93.97, seed 2, 96.60 / 93.62, +2.7% and +4.4%.
  - Each format thread's output reserved once, 64 bytes a line, instead of grown, with the same
    lines: seed 1, 92.03 / 91.27 → 91.90 / 91.13, 92.38 / 90.80 → 91.89 / 90.42 (31 runs) and
    91.96 / 90.90 → 92.23 / 90.54; seed 2, 92.38 / 90.97 → 92.09 / 90.96 and 92.43 / 91.37 →
    92.13 / 90.85: -0.2% on average, within the noise; memory.peak median 39.5 → 39.6 MB over
    five alternating runs; gate 510 pass. With the plan and the queries also freed on the
    thread that frees the chunks: seed 1, 91.86 / 91.00, seed 2, 92.16 / 91.04.
  - Not built: the terms deduplicated by each parse thread before the intern, as seed 1's 12
    shares hold 115,788 distinct terms between them against 152,904 occurrences, so the main
    thread would intern 24% fewer for a table per parse thread; and an intern on every thread
    into the shared table by compare-and-swap, each thread taking ids `t + threads * k` and the
    renumbering skipping the gaps, about 3 ms off the start for some 30 to 40 lines, which then
    also needs the term table set up during the parse (step 55) and the prefaulting threads
    moved out of the intern's time.

- **After step 61** (seed 1, `perf stat -r 5`, interleaved; `ab.py`, 31 alternating runs, medians /
  minimums, ms):
  - On step 62's packing, the probes holding the home slot's offset in bytes instead of its
    address, which the line end's loop, with three cursors gone, has a register for: the
    tokenizer's `lea` per token goes, 3,892.7 → 3,860.2 M instructions, but cycles 2,109 and
    2,110 → 2,086 and 2,120 M, and seed 1, 92.98 / 91.01 → 93.44 / 91.70; seed 2, 92.69 / 91.18 →
    93.73 / 91.94. The tokenizer still reloads its block's address from the stack per token.

- **After step 60** (on step 58, `ab.py`, medians / minimums, ms; timelines from builds with timestamps,
  seed 1, medians of 12 runs):
  - `parallel` as a tree: each thread spawns the upper half of its share and runs the lower
    half, so the last of 12 starts after 4 spawns, for 4 more lines. With `work` as a `dyn Fn`,
    61 runs, two rounds: seed 1, 98.73 / 97.02 → 98.37 / 95.13 and 98.02 / 96.62 → 98.25 /
    96.12; seed 2, 98.51 / 97.00 → 98.08 / 96.20 and 97.93 / 96.22 → 97.75 / 96.15 (-0.4% to
    +0.2%). Generic over `work` and a `Range`, scan's line end compiles inside `parallel`'s
    last step instead of the spawned closure, and keeps 0 in a register for the probe's
    `cmove`: one instruction less per token, 4,280.6 → 4,242.4 M instructions, cycles 2,229 and
    2,269 → 2,236 and 2,235 M. Four rounds (41 or 61 runs): seed 1, -2.3%, -1.2%, -0.9%, -1.0%;
    seed 2, -1.2%, -1.0%, -1.4%, -1.4%. On top of step 59 and this step it still saves the
    instruction (4,178.7 → 4,136.3 M, cycles 2,222 and 2,226 → 2,208 and 2,214 M), but the
    wall no longer moves: seed 1, 96.53 / 95.01 → 96.47 / 94.50 and 96.66 / 95.53 → 96.49 /
    94.15; seed 2, 96.76 / 95.06 → 96.38 / 93.58 and 96.36 / 95.24 → 96.43 / 94.64. Not kept.
    The token loop sets a register to zero for each of three selects per token; held in a
    register across the loop, one of them would go.
  - The corpus pages dropped in pieces of 2 MiB or 512 KiB, so that the other threads' frees,
    which wait on the memory-map lock while a drop holds it, get in between: the last thread's
    window freed in 0.27 or 0.07 ms instead of 0.70, but its drop took 2.33 or 1.89 ms instead
    of 0.64, behind those frees; wall medians 98.66 → 98.72 and 99.99.
  - A smaller sample, `SAMPLE_SHARE` 96 or 128 instead of 64 (41 runs): seed 1, 98.22 / 96.80
    → 98.44 / 96.90 and 98.40 / 97.21; seed 2, 98.27 / 96.60 → 97.75 / 96.86 and 98.18 / 96.73.
  - Bounds on the intern (2.87 ms on step 58, wrong answers): without the length of each key
    2.69 ms, without the `MORE` flag on the home slot 2.69, without the long key's store 2.93,
    without all three 2.46. The rest is the probe and the loop, about 100 instructions per
    occurrence with a dozen stack spills, and no single hot spot.

- **After step 59** (seed 1; `perf stat -r 5`, interleaved; `ab.py`, 61 alternating runs,
  medians / minimums, ms):
  - The zero that step 61 holds in a register, for `add_checks`'s select and its first-match
    cap too: cycles unchanged, as its loop then reloaded `seen`'s address from the stack. A long
    token's key of 0xFF bytes held in a register: its `vpcmpeqd` became a register move, cycles
    unchanged (step 61 moves it off the common path instead).
  - The probes' room for 32 more tokens, tested once per block, grown by a cold function: 21 M
    fewer instructions, cycles 2,200 to 2,204 → 2,198 to 2,202 M.
  - The probes holding the home slot's offset instead of its address: the token loop then kept
    one of its cursors on the stack. Not timed.
  - The gather masked to the real terms' lanes, with a pad id of `u32::MAX`, whose top bit
    drops its lane: +28 M instructions, cycles 2,212 to 2,218 → 2,225 to 2,230 M.
  - A line's token count added once per block (the popcount of its ends), and the probes
    written through a pointer, so that the token loop keeps no count: 7 M fewer instructions,
    cycles 2,223 to 2,225 → 2,236 to 2,244 M.
  - The phrase triggers' `phrase_at` entries prefetched after the token loop, and their first
    phrase entries after the triggers' loop: +7 M instructions, +0.3 M branch misses, cycles
    2,220 → 2,240 to 2,246 M.
  - Of the 12.94 M checks run, 9.20 M hit (71%), and 4.59 M (35%) hold whenever their trigger
    is present (`table & HAS[j] == HAS[j]`, `j` the trigger's lane), so that their gather tells
    nothing; 91% have four distinct terms or fewer. Not built: those checks without the gather
    (a second range per trigger would cost the triggers' loop more), or two checks of four
    terms in one eight-lane gather.
- **After step 58** (seed 1 unless noted; `perf stat -r 5`, interleaved; `ab.py`, 61
  alternating runs, medians / minimums, ms):
  - Where the loads wait, by source line: of the 6.0 M loads that miss L2, `add_checks` takes
    40%, the phrase candidates' loop 20% and the log's `store` lookup 16%; of the 38.7 M loads
    that hit a fill buffer (a load under way), `add_checks` takes 35% and the probe's slot load
    26%.
  - The table's slots marked by `threads` scoped threads, a share each, instead of on the main
    thread: seed 1, 99.84 / 98.64 → 99.55 / 98.42; seed 2, 99.54 / 98.37 → 99.31 / 97.80
    (-0.3%), for 5 more lines.
  - Each doc's checks run at the next doc's line end, so that the lines its triggers prefetch
    have a whole line of tokens to arrive: two of `seen`, `todo` and `kept`, swapped per doc.
    Fill-buffer hits 39.0 → 51.7 M and L1 load misses 45.7 → 59.1 M, as the lines left L1
    again before their use; cycles 2,255 to 2,259 → 2,250 to 2,283 M. With the triggers' prefetches
    into L2 only (T1): 2,250 to 2,276 against 2,260 to 2,261 M.
  - The log's entries without their `store` lookup (wrong answers, as a bound): cycles 2,252 to
    2,258 → 2,230 to 2,235 M (-1.0%). Too little for entries keyed by term id, whose counting sort
    at each flush would run over 75,748 starts instead of the 3,452 lists'.

- **After step 57** (`ab.py`, 61 alternating runs, medians / minimums, ms):
  - Each scanning thread groups its 121,600 or so first matches at its end in 1.2 to 3.8 ms: 0.3
    ms of page faults on the grouped ids, 0.1 to 1.4 ms allocating the 50,001 starts, 0.2 ms
    counting and 0.5 to 0.8 ms placing them. With no grouping at all (wrong answers): seed 1,
    101.07 / 99.70 → 98.09 / 96.43. The first matches pushed into buckets of 256 queries
    instead, and each output thread grouping its buckets of every chunk into ten ids per query:
    seed 1, 103.32 / 100.65 → 106.08 / 102.99; seed 2, 103.33 / 99.70 → 104.72 / 102.20; and
    memory.peak 38.1 to 39.6 → 48.0 to 48.4 MB, with 196 growing vectors per thread.
  - The corpus prefetched ahead of the walk, a line per block, 128, 512 or 4,096 bytes ahead:
    cycles 2,296 and 2,314 → 2,313 to 2,329 M, no change.
- **After step 56, the per-token work** (`perf stat -r 5`, interleaved, seed 1; `ab.py`, 41
  alternating runs, medians / minimums, ms):
  - The probe's miss test as a branch-free select in `find_from`, the miss mask hidden from
    LLVM, which made branches of it: 22 to 33 M fewer instructions, cycles -0.5% at most and none
    on top of the 32-byte checks below. Seed 1, on step 55's code, 104.05 → 103.75.
  - A token's length clamped to 33 in `fold`, with a 0xDF byte before the mask's 0xFF bytes, so
    that a token past 32 bytes takes a key no term has without an `Option`: 74 M fewer
    instructions, cycles +0.3%; 104.05 → 104.15.
  - No prefetch of a token's `check_at` entry: 37 M fewer instructions, cycles mixed; 104.05 →
    103.67, within the noise.
  - A [`Check`] of 32 bytes, its query index moved to a vector of its own read only for first
    matches and at the pass's end: 7 M fewer instructions, cycles 2,357 to 2,362 → 2,342 to 2,348 M
    (-0.6%). With it, `check_at` and `phrase_at` side by side, one prefetch per token for both:
    +16 M instructions, cycles -0.4% to -0.8% more; plus a prefetch loop over the phrase
    entries, +51 M instructions and +0.3 M branch misses for about -0.5%; with the term's store
    as a third column, +95 M instructions, slower. The 32-byte checks with the pair, against
    step 56: seed 1, 104.85 / 103.12 → 104.23 / 102.44; seed 2, 105.79 / 104.12 → 105.37 /
    103.63. On top of step 57's log (its `u64` form): cycles -1.6% (2,310 and 2,305 → 2,273 and
    2,237 M), but seed 1, 102.86 / 101.58 → 102.52 / 101.59; seed 2, 103.02 / 101.99 → 102.70 /
    100.88; seed 3, 102.93 / 101.35 → 102.84 / 100.88 (61 runs, -0.1% to -0.3%), for 8 more
    lines.
  - A prefetch of a first match's `shown` count in `add_checks`: 20 M more cycles. The cap on
    `shown` dropped, the first matches extended unconditionally: 10 to 41 M fewer cycles,
    mixed, and about 3 MB more at the memory peak.
  - Bounds (wrong answers, to size each stage): with no phrase checks, 95 M fewer cycles
    (-4.1%); with no first matches counted, 125 M fewer (-5.4%), mostly their grouping in
    the tally.

- **After step 54, the serial start and the end of the pass** (local, seed 1 unless stated;
  timelines from builds with timestamps, ms since `main`; `ab.py` medians / minimums, ms):
  - Prefaulting threads, by count (20 alternating runs each, one build, wall medians): none
    105.62; 3 threads 104.05; 4, 104.30; 5, 104.42; 6, 104.89; 5 reading a byte per 4 KiB,
    104.57; 5 started at `main`'s start, before the parse, 106.87 (they slow the parse). Seed 2,
    30 runs: none 105.56; 2 threads 105.39; 3, 104.35; 4, 104.48. 11 threads at `SCHED_IDLE`,
    105.36; 5 at `SCHED_IDLE`, 104.55.
  - `MADV_SEQUENTIAL` on both mapped files, for a larger fault-around: 105.75 / 104.24 →
    105.82 / 103.73 (41 runs).
  - Each format thread freeing its own chunk after a `Barrier`, so that none frees a chunk
    another still reads: 105.92 / 104.40 → 106.75 / 104.89. Twelve threads unmapping at once
    contend for the memory-map lock and its TLB shootdowns.
  - The plan's temporaries freed on a detached thread, on top of the prefaulting: 104.63 /
    103.18 → 104.68 / 102.77.
  - The scanning threads' corpus pages not dropped (`MADV_DONTNEED`) at the end of the walk: on
    the last thread to finish only, the wall 106.72 → 106.42 but the exit 0.22 ms longer; on the
    last two, 107.35; on all, 109.64.
  - Not built: the term table's 4 MiB written a word per 4 KiB instead of whole would save about
    0.12 ms of its 0.68; marking the slots on more threads at most 0.26 ms, as the grouping
    thread ends at 11.09 to 11.21 against the main thread's 11.73.

- **After step 55, the scanning threads' setup** (`ab.py`, 41 alternating runs against step
  55, medians / minimums, ms):
  - All of the tallies' cells on huge pages ([`huge`]), one block for the 12 tallies made by a
    helper thread during the plan's tail, each scanning thread taking a ready `Tally`: the walks
    start at 11.58 to 12.03 instead of 12.02 to 12.63 ms (the helper ends 0.2 ms after the tail), and
    the walks are a little shorter. Seed 1, 105.58 / 103.65 → 104.42 / 103.12; seed 2, 106.10 /
    104.48 → 104.71 / 103.34; seed 1, 105.36 / 104.04 → 104.43 / 102.98; seed 2, 105.97 /
    104.83 → 104.46 / 103.59; seed 3, 106.39 / 104.75 → 105.22 / 103.64: -0.9% to -1.4%. But
    memory.peak 38.8 → 44.2 MB. The peak falls at the end of the scans, where each tally's
    check cells, a vector of their own, went back to the system at its merge; on huge pages they
    cannot, as a part of a huge page given back stays charged until the page is split, and the
    whole pages round 8.7 MB of cells up to 10.5. With the check cells in a block of their own,
    given back whole once every scan is done: 43.5 to 44.8 MB, as the peak comes before that.
  - Only the query cells on huge pages: 105.36 → 105.85 and 105.97 → 105.90, and +1.6 MB. Only
    the check cells: seed 3, 106.39 / 104.75 → 105.88 / 104.28, and +4.6 MB.
  - The plan's per-thread check lists freed on the grouping thread, which now owns them, with
    the BOOL flags set before it starts: in the timeline the plan ends 0.46 ms sooner, but seed
    1, 105.67 / 104.71 → 105.83 / 104.82 and seed 2, 106.16 / 104.25 → 106.47 / 104.62. With
    the tallies also made on 4 KiB pages during the tail, by 12 threads the grouping thread
    spawns: 105.32 / 103.77 and 105.48 / 104.28, -0.3% and -0.6%.

- **After step 54, more of the serial start** (local, seed 1, builds with timestamps, ms since
  `main`, medians of 10 to 12 runs, unless stated):
  - The terms counted on the main thread with `split` per line and a `starts_with` per line: 3.6
    ms, as the 13th thread on 12 CPUs. Counted with `filter().count()`, a `usize` per pair: 0.44
    ms against 0.23 for the `u32` sum on a quiet core, and the table ready 0.28 ms later.
  - The count and the table on a thread of their own, or each parse thread counting its share
    and sending it over a channel: the intern ends at 5.13 to 5.23 ms against 5.21, for 3 to 5 more
    lines. With 12 parse threads instead of 11, the count and the table competed for a CPU:
    5.53 to 5.64 ms. With 10, 5.23 ms.
  - The scanning threads' `Tally` cells written during the plan's tail, by 12 threads that the
    grouping thread spawns, each scanning thread taking one: its walk started 0.5 ms sooner, but
    the main thread's tail ran 0.4 ms slower with the page faults beside it, and `ab.py` (41
    alternating runs, seeds 1 and 2) drew medians 106.04 and 106.26 ms against 105.94 and 106.21
    without it, for 7 more lines.
  - `lens` written at each id in an array sized like `keys`, instead of pushed and cut back:
    `parse_queries` alone, 40 times in a loop, 4.72 to 4.85 → 4.69 to 4.71 ms, for 2 more lines.
  - In a microbenchmark, 12 threads each writing 726 KB of new cells (a `Tally`): the slowest
    took 0.81 ms; zeroed by `calloc` and faulted in by `MADV_POPULATE_WRITE`, 1.27 ms. A 4 MiB
    block on huge pages, one thread: `alloc` and `write_bytes` 0.71 ms, `mmap` and a write per
    page 0.59 ms; the kernel's zeroing of the huge pages is most of it.
  - Not built: an intern on every thread, each taking the terms whose home slot is in its part
    of the table and deferring a probe that runs past it, then every thread reading its share's
    ids back from the table. About 0.7 ms a thread against 2.9, but three parallel steps and
    their joins on the main thread, and about 40 lines.

- **After step 53, the rows renumbered after the plan instead of before it.** The same pass as
  step 54, but the plan numbered the rows first and then renumbered the checks (on the grouping
  thread, which sent the BOOL flags back over a channel), the phrases, the queries, the long keys
  and the stores. Seed 1, 108.98 / 107.70 → 106.88 / 105.37 ms, as fast as step 54, but S 789 →
  845. Renumbering before the plan with new `keys` and an extra pass over the slots: S 818, but
  2.3 to 3.1 ms on the main thread, 1.5 ms slower than step 54.
- **After step 53, the rows set from a per-doc log of events**: each token's flags pushed to one
  event list, walked at the line end. Instructions +186 M, cycles +1.4%.
- **After step 53, the rows' read-modify-writes through a 64-doc buffer**, without the
  renumbering (each row term's bit gathered from `seen` by its id): cycles the same, instructions
  +24 M. With the spill's writes into the rows left out (wrong answers, as a bound) the cycles
  did not move either: the rows' stores are not what costs, the per-term work is.
- **After step 53, the phrases grouped on a thread of their own**, and the sample's `hits`
  reused for the renumbering: no change in the time to the end of the plan.
- **After step 52, line cuts that moved the pass** (local, seed 1, `perf stat -r 5`). The
  scanning thread's code is one function, so a cut in its cold start or end can change how
  LLVM allocates the hot loops' registers. Each of these added instructions and was put back:
  `Tally::take_first` inlined at its one call, 4,829.8 → 4,853.8 M (`group` then inlines into
  the scan); `group_into`'s counts and prefix sums as `for_each` one-liners, 4,828.5 → 4,852.1 M,
  and 4,851.2 M with `take_first` kept out of line; the checks' cells added to their queries'
  by one `for_each` after the walk, 4,828.5 → 4,856.8 M. In the walk's token loop, the block's
  first start dropped by `starts &= select_unpredictable(carry, !0, starts - 1)`: 4,828.7 →
  4,832.9 M, cycles the same. Each would have saved 1 to 4 lines. `#[target_feature(enable =
  "avx2")]` cannot go either: the build's `target-cpu` does not make the intrinsics safe to call.
- **After step 52, where the cycles and the wall go** (local, seed 1, turbo off). Topdown: retiring
  55%, bad speculation 14%, frontend 13%, backend 18%, of which memory about 8%; the kernel takes
  6.6% of all cycles. A build with timestamps, in ms since `main`: the queries are parsed and
  interned at 6.0 to 6.6, sampled at 8.7 to 8.9 and planned at 11.8 to 12.4, and the walks start at
  12.8 to 13.5. A walk takes 85.6 to 89.5 ms, and the slowest chunk is a different one in each run (end
  spread 2.7 to 4.1 ms). The last thread's tail takes 2.3 to 4.3 ms: last flush 0.4 to 0.6, the window's
  free 0.2 to 1.5, merging the cells 0.2 to 1.0, grouping 1.1 to 2.4. Formatting then takes 1.5 ms and
  the write 0.5. Cycle cuts in the pass reached the wall at about a third or half of their size,
  as below.
- **After step 52, frequent terms moved into their home slots.** After the sample, a term stored
  past its home slot swaps with the term there when it was sampled in more documents. The swap
  keeps every probe's run free of empty slots, and each slot keeps its `MORE`. Term tokens probed
  past their home slot fell from 1.10 to 0.48 M; the rest are a long tail of terms whose home
  holds a more frequent term. Branch misses went 11.4 → 10.6 M, instructions 4,829.8 → 4,800.2 M,
  cycles 2,499 and 2,521 → 2,477 and 2,477 M. Run over all 262,144 slots, the settle took 1.57 ms
  on the main thread, the plan's critical path, and the wall moved -0.2%. Over only the 11,026
  slots the intern recorded, it took 0.18 ms. `ab.py`, medians / minimums: seed 1, 109.01 /
  107.80 → 108.47 / 107.13 ms; seed 2, 110.11 / 107.92 → 109.80 / 106.95 ms; seed 3, 61 runs,
  109.66 / 108.12 → 109.09 / 107.56 ms (-0.3% to -0.5%), for about 20 lines.
- **After step 52, the rows' loop** (local, seed 1, `perf stat -r 5`, interleaved; plain `b-0`
  drew 2,482 to 2,526 M cycles). The rows hold 532 of 75,748 terms, and a doc sets 45 bits in them.
  With the loop skipped (wrong answers), L1 load misses went 81.3 → 57.0 M and cycles fell 1.3% to
  2.8%. With a row picked from the term id instead of `store` (wrong answers), L1 load misses went
  81.3 → 70.6 M and cycles fell 0.5% to 1.0%. None of these paid:
  - Each row one line longer (576 bytes), so that a doc's words in the rows spread over all 64 L1
    sets instead of 8: L1 load misses 80.6 → 82.1 M, cycles 2,493 and 2,490 → 2,513 and 2,517 M.
    Held to 8 sets, the rows did not evict the rest of L1.
  - `store` beside `check_at`, as one `Vec<(u32, Store)>`, so that the token loop's prefetch of
    `check_at` brings each term's store: L1 load misses 81.2 → 75.1 M. But the token loop then
    reloaded `kept`'s address and kept a second cursor on the stack: instructions 4,829.8 →
    4,888.1 M, cycles 2,518 and 2,528 → 2,546 and 2,548 M.
  - The stores of a doc's row terms prefetched before the BOOL triggers' loop: L1 load misses
    81.3 → 76.1 M, instructions +56 M, cycles 2,482 and 2,524 → 2,551 and 2,543 M.
- **After step 52, `check_at`'s prefetch with another hint**, T1, T2 or NTA instead of T0: cycles
  2,525 and 2,524 M → 2,525 and 2,526 (T1), 2,525 and 2,523 (T2), 2,549 and 2,549 M (NTA).
- **After step 52, `DENSE_SHARE` 56, 64 and 80** (`ab.py`, 41 alternating runs, seed 1): medians
  109.02 → 108.92, 108.45 and 108.61 ms; 64 on seed 3 (61 runs): 109.66 → 109.22 ms (-0.4%),
  for about 1.9 MB more at the peak (step 44's measure).
- **After step 52, the tail's blocks released by `MADV_DONTNEED`** and never freed (the window's
  bits, the check cells and the first-match list), so that no `munmap` takes the address space's
  lock for writing, without reserving `at` and `ids` before the walk as the experiment after step
  49 did: medians 110.60 → 112.26 and 109.25 → 110.32 ms (+1%).
- **After step 51, token starts and ends relative to their block** (local, seed 1, `perf stat -r 5`,
  interleaved), so that the walk's token loop adds no block offset to either and `key_in` loads
  from the block's address: instructions 4,875.7 → 4,887.2 M, as the loop then held the block's
  offset as well as its address and kept the window's bound on the stack. With the block's
  address standing for its offset through the whole walk, 4,859.9 M and cycles 2,538 → 2,530 M,
  the bound still on the stack, as LLVM kept the chunk's address and length in registers for the
  token near the chunk's end; with that token's key out of line, the call cost the loop its
  registers: 5,221.5 M instructions, cycles 2,634 M.
- **After step 51, `classify` from two nibble tables** (`_mm256_shuffle_epi8`, as simdjson does):
  per half block, a shift, a mask, two lookups, an AND and a compare instead of nine operations.
  Same answers (gate 510 of 510). Instructions 4,875.9 → 4,851.6 M, cycles 2,546 → 2,536 M over
  three rounds; `ab.py`, 41, 41 and 61 alternating runs on seeds 1, 2 and 3: medians 109.51,
  110.53 and 110.30 → 109.36, 111.01 and 109.93 ms, for 4 more lines.
- **After step 51, three more cuts in the line end's token loop** (local, seed 1, `perf stat -r 5`,
  interleaved). The long keys' side of the probe marked `std::hint::cold_path()`, which dropped
  the jump that the short side took at every token: instructions 4,875.4 → 4,838.5 M, cycles the
  same over five rounds (2,510 to 2,547 against 2,509 to 2,543 M), `ab.py` median 109.78 → 109.94 ms.
  The probes past a home slot listed in a sixth run by a store and an add and run after the loop,
  with the per-token work in a closure that both loops call: the closure's cursors stayed in
  memory, loaded and stored at every token, and branch misses rose 11.38 → 19.53 M; cycles 2,501
  → 2,890 M. `find_past` inlined, with the table behind `black_box` so that its address and size
  are read only on that path: instructions 4,875.6 → 4,923.9 M, cycles 2,538 → 2,544 M.
- **After step 51, the probes' buffer grown only when it is short** (`if probes.len() < n + 32`),
  instead of `resize` to the larger length at every block: instructions 4,875.3 → 4,870.6 M,
  cycles 2,538 → 2,539 M, for 2 more lines.
- **After step 50, a token past 32 bytes keyed 0 by a conditional move** (local, seed 1), instead
  of a branch to a key of 0xFF bytes: a length of 0 masks every byte off, and the key 0 probes to
  an empty slot, id 0. Instructions 4,949.7 → 4,944.9 M, but cycles 2,584 and 2,584 → 2,630 and
  2,635 M (+1.8%): the walk's token loop, one register short, then rebuilt the hash's constant at
  every token.
- **The scanning threads' tails mapping and unmapping nothing** (on step 49). The tail's wall time was
  1.5 to 6 ms against 1.3 to 2.2 ms of CPU: each `munmap` and `mmap` waited on the memory-map lock while
  other threads dropped their 21 MB of corpus pages with `MADV_DONTNEED`. Releasing the window bits,
  check cells and first-match list by `MADV_DONTNEED` instead of freeing them, and reserving the
  grouping's `at` and `ids` before the walk, took T down 0.89 to 1.49 ms in six rounds of 41 (seeds 1
  and 2), and 114.47 → 113.56 ms on seed 3. But with no lock waits the threads group at the same
  moment: memory.peak on seed 3 went 38.5 to 39.4 → 39.5 to 42.5 MiB, and 42.5 MiB is over B's 44.0 MB
  cap before the board's extra MB. For 12 lines, and `scan` past clippy's line limit, the net was
  about zero. Not kept.
- **After step 48, the check cells dropped** (local, seed 1). With the first matches halved,
  memory peaks as the threads group them, about 1.5 MB above the walks (in about half the runs,
  33.0 against 34.6 MB), and the 326 KB of check cells per thread are freed by then. A check
  then added its match to its query's cell, a random line of 400 KB, instead of its own cell
  beside its neighbours: 8 lines fewer, but cycles 2,712 and 2,649 → 2,830 and 2,793 M, `ab.py`
  (41 alternating runs) median 117.24 → 120.90 ms, memory.peak median 35.0 → 32.4 MB over 5,
  about even at the board's rates. With the query cell prefetched six checks ahead: cycles 2,705
  and 2,709 → 2,741 and 2,735 M, medians 116.63 and 117.55 → 117.61 and 117.84 ms, memory.peak
  median 34.7 → 33.85 MB over 6, again about even.

- **After step 45, glibc's mmap threshold raised** from 128 KiB to 512 KiB or 2 MiB, so that the
  tail's frees and allocations reuse the arena instead of queueing on the address space's lock
  (`ab.py`, 21 alternating runs): medians 118.82 → 118.28 and 118.37 ms, but memory.peak over 5
  runs, median 37.6 → 42.7 and 44.5 MB.

- **After step 44, where the time goes at turbo off** (local, seed 1, builds with timestamps,
  12 runs, in ms since `main`). The walks start at 10.5 to 12.7 ms and end over a spread of 2.2 to 4
  ms (104.4 to 112.1 ms); in one run of twelve one thread ended 5 ms after the rest, which turbo on
  showed in half the runs (below). After its walk, a thread spends 0.35 to 0.95 ms dropping its
  pages, 0.5 to 0.8 ms on its last flush, 0.1 to 1.2 ms freeing its window, 0.15 to 1.25 ms merging its
  cells (the free included) and 1.2 to 3.1 ms grouping its 120,700 or so first matches, of which
  0 to 1.1 ms is allocating `ids`, 0.1 to 1.2 ms allocating `at`, 0.15 to 0.36 ms counting and 0.8 to 1.6
  ms placing them, page faults on `ids` included: about 3.5 to 5 ms in all, whose spread is mostly
  the frees and allocations queueing on the address space's lock. Formatting then takes
  1.3 to 1.8 ms and the write 0.5 to 0.7 ms. `perf record` with the kernel's samples: 5.3% of all
  cycles run in the kernel, two thirds of them in page faults, half of those on the corpus
  mapping. Not taken further: the work is the same on every thread, so only a dynamic handout
  of the corpus would win back the walks' spread, about 1.3 ms above their mean.

- **After step 44, the serial parts and the tail** (local, seed 1, turbo on, so a run takes
  about 69 ms; `ab.py`, 41 alternating runs, two rounds, medians, step 44 first). A build with
  timestamps (6 to 10 runs, ms since `main`): the plan is ready at 6.6 to 9.2, the walks end over a
  spread of 3 to 5 ms (62.0 to 68.7), and after its walk a thread spends 0.30 to 0.40 ms dropping its
  pages, 0.67 to 0.79 on its last flush and freeing its window (0.40 to 0.51 of it the frees), 0.25 to 0.48
  merging its cells and 1.18 to 1.47 grouping its first matches; formatting takes 0.78 to 0.96 ms,
  spawns included, and the write 0.40 to 0.45. In 3 of its first 6 runs two scanning threads ended 15 to 28
  ms after the others, as if they had shared a CPU. On CPUs 0, 2, 4, 6, 8 and 10 alone (6
  threads, one per core) a run takes 77.18 ms against 70.42 on all 12. None of these paid:
  - Each parse thread starting a thread of its own that reads a byte per page of a twelfth of
    the corpus while the terms are interned: 71.23 and 69.87 → 71.28 and 69.63 ms.
  - The last window flushed over the words its documents fill only (the flush 0.2 ms): 69.55 and
    69.80 → 69.53 and 69.66 ms.
  - Each scanning thread pinned to a CPU of its own (`sched_setaffinity`): 69.78 and 70.13 →
    69.96 and 70.01 ms (means 70.49 and 72.14 → 70.18 and 71.07).
  - The answers formatted by the scanning threads after a barrier, instead of new threads:
    69.21 and 69.37 → 69.17 and 69.48 ms. With the two above: 69.34 and 69.38 → 69.07 and 69.14
    ms, -0.35%, for about 20 more lines.
  - Every parse thread interning its own terms into the shared table, claiming an empty slot by
    a compare-and-swap of its flags and taking ids from one atomic counter: interning 2.0 → 4.4
    ms, as that counter's cache line moves between cores at each new term, and the plan 0.6 ms
    slower with ids in no file order: 69.78 and 69.64 → 72.86 and 72.23 ms.
  - `DENSE_SHARE` 16, 48, 64 and 128 instead of 32: 16 +3.0 ms and 128 +1.4 ms; 64 -0.14, -0.60
    and -0.62 ms, but memory.peak median 40.1 → 42.0 MB over five runs; 48 -0.94, -0.66, -0.69
    and -0.76 ms in four rounds (-1.1%), but 40.1 → 40.95 MB over ten (+2.1%), about even at the
    board's exchange rates. With 48 for which queries run on sets and 32 for which terms keep a
    row, or the other way round: -0.11 ms each. `WINDOW` 32 or 128: +0.26 and +0.09 ms; 32 with
    `DENSE_SHARE` 48: -0.23 ms. `SAMPLE_SHARE` 32 or 128: +0.68 and +0.10 ms.
  - A second prefetch per BOOL trigger, of the last byte of its checks, as a 40-byte check ends
    on the next cache line half the time: 69.30 and 69.11 → 70.02 and 69.83 ms.
  - Without the drop of each chunk's corpus pages (step 17): 68.77 → 70.37 ms, so it still pays.
    The exit itself, from just before `exit` until the parent sees the process end, takes 1.14
    ms (median of 15, min 0.92).

- **After step 43, the prefetch for BOOL triggers only** (local, seed 1). The token loop
  prefetched the entry of a token with the BOOL flag and entry 0 otherwise, picked by
  `select_unpredictable`: instructions 5,412.1 → 5,557.2 M, cycles 2,891 and 2,893 → 2,913 and
  2,876 M against prefetching every token's entry, for 2 more lines.

- **After step 43, bounds on two other targets** (local, seed 1, `perf stat -r 5`). Skipping the
  loop that sets the rows' bits saved 10 M cycles of 2,909, 0.35%; skipping the flushes of
  windowed bits, `bit_index_sum` among them, saved 16 M, 0.55%. Neither was taken further. A
  probe that stops at a non-term token's occupied home slot would need a bit set by `intern`;
  at +3 to 4 instructions per token against about 2.5 M fewer mispredicts it was not tried.

- **After step 41, the line end as a call of its own** (local, seed 1). `walk` called its
  callback through a helper that is never inlined, so that the token loop would have the
  registers: instructions 5,796 → 6,538 M, cycles 3,172 → 3,345 M, as the line end then reads
  everything it captures through the closure, again after each store.

- **After step 42, a write pointer instead of the token count** (local, seed 1). The token loop
  bumped a pointer, and the count came from its offset after each block: instructions 5,382.2 →
  5,358.7 M, cycles 2,992 → 3,018 M, and `ab.py`, 41 alternating runs, two rounds, medians 129.04
  and 131.86 against 129.11 and 127.90 ms, minimums 123.38 and 124.63 against 124.79 and 124.89
  ms, for 2 more lines.

- **After step 39, each token's range of checks prefetched** (local, seed 1). The BOOL triggers'
  loop stalled on its load of `check_at[t]`, a 303 KB array read at random: 4.4% of all cycle
  samples sat on that load and the prefetches that wait for it. The line end's token loop then
  prefetched it for each token. Picking the entry of a new BOOL trigger, or entry 0, by a
  conditional move, on step 38 with step 41's fold: instructions 6,054 → 6,329 M and cycles
  3,312 → 3,353 M, as the loop spilled registers. Prefetching every token's entry: 6,147 M
  instructions and 3,247 M cycles there, but on step 41, `ab.py`, 41 alternating runs, median
  131.77 against 131.65 ms without it, minimum 129.69 against 129.84 ms.

- **After step 38, on the serial start** (local, a build with timestamps, seed 1, medians of 10
  runs, in ms since the process was spawned; `main` starts at 2.0, as a trivial static binary's
  does). The main thread alone spawns the parse threads (2.16 to 2.59), allocates the term table
  (0.94), interns the terms (3.02, 126 user instructions per occurrence at 4.3 per cycle), sums
  the sample (0.59), groups the phrases (0.26) and the checks (1.34), sets the stores (0.28),
  marks the table (0.73) and frees the planning's lists (0.18): about 8.5 ms, against about 4.5
  ms in which every thread works. Each scanning thread then takes 0.8 to 1.0 ms to set up before
  its walk begins at 15.2 to 15.7 ms. None of these changes paid:
  - The intern in a function of its own: 3.02 → 3.35 ms, and the pass about 4 ms slower in the
    same build, from the moved code.
  - The terms' keys touched before the intern, or prefetched 48 keys ahead: 3.05 ms.
  - The term table zeroed by the kernel, `mmap` and `MADV_POPULATE_WRITE`: 0.85 → 0.86 ms.
  - The table marked through the slot positions the intern recorded: 0.61 → 1.37 ms.
  - The corpus faulted in ahead of the pass. With `MADV_POPULATE_READ` on the main thread:
    9.1 ms, for a pass about 2 ms shorter. From helper threads, the intern waited about 10 ms on
    the `mmap_lock`; per chunk in the scanning threads, 9 to 10 ms, serialized on the same lock.
    Helper threads reading a byte per page at nice 19: the parse, sample and plan about 2 ms
    slower, the pass about 1.6 ms faster, the wall 138.41 → 138.00 ms.

- **After step 33, on the serial parts** (local, a build with timestamps, seed 1). The pass
  starts at 15.4 ms: parsing the queries takes 7.7 ms, of which interning 3.4, the sample 3.5,
  planning the queries 2.9 and marking the term table 1.1 ms. The threads' passes run 129.5 to
  131.6 ms. At its end, a thread spends 0.3 to 1.1 ms dropping its pages of the corpus, 0.5 to 0.75
  ms on its last window flush, 0.3 to 1.25 ms freeing its sets, 0.2 to 0.9 ms merging its cells and
  1.3 to 2.3 ms grouping its first matches. Formatting takes about 1.9 ms, the frees after it 1.3
  ms and the write 0.5 ms. None of these changes paid:
  - The intern loop's prefetch 16 terms ahead (dropped at step 21) put back, or 8 or 32 ahead:
    interning took about 0.35 ms less, for 3 more lines.
  - The queries, plan and chunks leaked instead of freed after formatting, as after step 17:
    three rounds of 20 runs, medians -0.96, +0.10 and -1.80 ms.
  - Nothing freed at the end of a thread's pass: median 152.28 → 152.27 ms, memory.peak 41.4 →
    50.8 MB.
- **PGO** (local, turbo off, on step 31's code): a profile from three runs on each of full seeds
  2 to 4, passed as `-C profile-use`, with every function matching it. `ab.py`, 16 interleaved runs,
  medians / minimums: 161.21 / 157.44 → 168.09 / 160.76 ms and 158.09 / 155.53 → 161.43 / 159.20 ms
  on seed 1; 158.11 / 156.41 → 161.79 / 159.80 ms and 163.53 / 156.80 → 168.75 / 161.82 ms on seed
  10, which it was not trained on. `perf stat -r 5`, twice: instructions:u 7.34 → 7.73 G, cycles:u
  3.83 to 3.87 → 3.84 to 3.93 G, branch-misses:u 18.1 → 17.8 M.
- **After step 17, on the serial start and the end** (local, alternating runs, 16 each, against
  step 17 at 238.5 to 239.6 ms median). Where the time goes on seed 1: 2.1 ms from spawn to `main`,
  parsing to 8.5 ms, the sample 5.3 ms, the per-query planning loop 5.2 ms and the rest of the
  plan 2.9 ms, so the pass starts at about 22 ms. The threads' passes end over a 4 to 9 ms spread.
  After its pass, a thread spends 0.6 to 0.8 ms on its last window flush, 0.1 to 0.3 ms each on
  freeing the window and merging the check cells, and 1 to 2.7 ms grouping its first matches.
  Formatting takes 1.5 ms, joining the workers and freeing the chunks, plan and queries 1.3 to 1.4
  ms, writing 0.6 ms, and the exit 0.8 ms. None of these changes paid:
  - The sample counted by all threads, one part each, then summed: the plan was ready 1.3 ms
    sooner (21.4 to 21.8 → 20.0 to 20.7 ms), but the wall did not move (239.6 → 240.1 and 238.5 →
    239.1 ms), for 16 more lines (kept at step 20). The sample's slots prefetched as in the pass
    instead (4 more lines) cut the sample from 5.3 to about 4.1 ms: 239.0 → 239.6 and 239.0 →
    240.3 ms (kept at step 28).
  - The queries, plan and chunks leaked instead of freed after formatting: the exit then took
    1.7 to 1.9 ms instead of 0.7 to 1.0, and the wall went from 238.5 to 240.6 ms.
  - The grouping's two arrays written when allocated, as in step 16: 238.9 → 239.5 ms. Each
    thread's first-match list reserved at its bound (`SHOWN_IDS` per query), so it never moves
    while it grows: 238.5 → 238.6 ms.
- **A static-pie build** (`+crt-static`, as tracks A and C use), on step 14 (local, turbo off):
  `ab.py`, 16 interleaved runs, median / min 246.00 / 245.60 ms against 245.75 / 244.05 ms. The
  dynamic loader's quarter of a millisecond is lost in a 245 ms run.
- **Transparent huge pages off on top of the fixed threshold** (`prctl(PR_SET_THP_DISABLE)`):
  41.1 to 41.2 MB against 42.4 to 45.3 in the same session, but cycles 9.79 to 9.83 → 9.91 to 9.97 G and
  elapsed 0.383 to 0.385 → 0.392 to 0.397 s. Huge pages off without the fixed threshold gave
  49.8 to 50.8 MB, and `malloc_trim(0)` after planning 49.2 to 50.1 MB.
- **Every BOOL query checked per document, with no sets at all** (step 3 without the sets):
  1.29 s, 29.0 G cycles, 53.3 MB, against 0.67 to 0.75 s and 14.0 to 14.2 G for step 2 in the same
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
  13.8 to 14.7 → 10.7 to 11.0 ms, but 8.4 MB of slots instead of 4.2, as the queries hold 152,904 term
  occurrences and only 75,747 distinct terms.
- **Huge pages without the `realloc` path**, every resize a new block and a copy: median wall
  288.9 ms against 295.5 for step 6 (285.3 with it). The parse and the plan grow their arrays
  by copying. With `alloc_zeroed` also sent to glibc's `calloc` as well: 288.4 ms.
- **Huge pages only for blocks of 4 MiB or more** (the term table and its keys): the same
  cycles and memory as 2 MiB.
- **No branch per token at the line end, on top of step 6** (kept at step 25). A token that
  is no query term resolved to an empty slot with a reserved id and no flags, and the trigger
  lists grew by writing each entry and advancing by 0 or 1. Branch misses fell from 77.6 to
  65.1 M, but instructions rose from 10.07 to 10.57 G and cycles stayed at 7.19 to 7.20 G.
  Prefetching the triggered checks after the token loop, or at first sight behind one branch,
  was 0.5 to 1% slower than step 6.
- **The full keys of long tokens prefetched** in a pass between hashing and probing, on a
  variant of step 5 that was never merged: cycles 8.40 to 8.44 → 9.09 to 9.12 G, as the extra pass
  stalls on the slot loads it reads. On step 6, skipping the long-key comparison altogether
  (wrong answers, as an upper bound) saved only 1.5 to 2.8% of cycles, too little for a prefetch
  scheme to chase.
- **Phrase triggers pushed without a branch** (each position pushed, then dropped again unless
  the term triggers phrases), on top of step 9: branch misses 68.8 → 67.2 M, but instructions
  10.06 → 10.44 G and cycles 6.80 → 6.87 G.
- **Every BOOL query on sets, a window at a time, on step 25** (a prototype). Common terms keep
  their bitset rows; for the others the pass logs each document they occur in, and at each
  window's end a counting sort groups the log by term into lists that every BOOL query is
  evaluated on, merging with galloping search. Correct on seeds 1 and 2; `ab.py`, 12 runs each,
  two rounds: median 179.82 and 179.46 ms against 183.80 and 183.09 ms, about 2% faster, but
  memory.peak over four runs 52.1 to 53.9 MB against 41.9 to 43.0 MB. Of its 58.8 ms of BOOL work,
  logging took 11.5, grouping 8.3 and evaluation 39.0: at least 55 million merge steps, where
  the per-document checks run 16.9 million times.
- **The query terms interned on every thread, after step 63** (local, turbo off). Eleven threads
  each parse a share of `queries.txt` and intern their terms straight into one shared table: a
  new term claims its empty slot by a compare-and-swap of the slot's `head`, takes its id from a
  block of 64 that its thread draws from an atomic counter, and publishes the id last (Release),
  which a probe that meets the same `head` waits for (Acquire). The pass's page-touching helper
  threads moved into the plan's tail, two thirds of the threads. Gate 512 pass, 0 fail; 12 runs
  of seed 1 byte-identical (8 on 12 CPUs, 4 on 4). `ab.py`, 41 interleaved runs: seed 1 median
  92.56 → 89.94 ms (min 91.58 → 88.79), seed 3 92.37 → 90.08 ms (min 91.15 → 89.12), about
  -2.6%. memory.peak, 3 alternating runs: 39.0 to 39.8 → 39.4 to 39.7 MB. But S 901 → 961 as pushed:
  K ×1.004, about +0.08 points, inside the local runs' spread. As step 62 ran 7% slower on the
  board than here, it went to the board as a trial (step 64), to be kept only if its runs beat
  step 63's 91.1 and 92.9 ms by more than their noise. Its run, run-bb4ed7d47695, read 93.4 ms,
  in a run where A, C and D read 1.5 to 3.4% above their best: no gain on the box against step 63,
  for 60 more lines. Reverted to step 63's code, whose next run, run-79b6065fa154, read 98.1 ms
  in a slower run still (A 18.3, D 133.6 ms). Step 66 tried it again and kept it.
