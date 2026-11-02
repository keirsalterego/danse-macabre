# Track C: performance log

Kept steps cite the board run that measured them. Local numbers come from my i7-13620H laptop with turbo off, comparing two versions side by side, and whatever I measured and dropped is listed under the step it followed.

| | Reference (step 0) | Final code (step 54), three board runs |
|---|---|---|
| T | 14.44 s | 19.34 ms (run-a66fe96b626a), 19.42 ms (run-dc3871163ade), 19.63 ms (run-a69fa3d057dc) |
| M | 367 MB | 11.3 MB, 11.4 MB, 11.6 MB |
| S (lines) | 108 | 348 |
| Points | 0 | 83.1, 83.1, 82.9 |

## Checking the headline

Points are 50 · log10 K, where K = (Tref/T)^0.5 · min(Mref/M, 50)^0.2 · min(Sref/S, 1)^0.15 · (Dref/D)^0.15. For run-a66fe96b626a: (14.44 / 0.019337)^0.5 = 27.33, (367 / 11.29)^0.2 = 2.006, (108 / 348)^0.15 = 0.839, and 1 for the dependencies. K = 46.0, and 50 · log10 46.0 = 83.1.

## What didn't work

- PGO (under step 27). Same instructions, 1.535 G, but cycles went 0.841 → 0.875 G and the median 36.35 → 37.71 ms on seed 1.
- Keeping the empty name out of the mailboxes. Its owner gets 11% more letters than the others, but not posting them measured the same (37.33 against 37.16 ms), and keeping one bit per country for it in the batch loop cost 2.2 to 2.3 ms.

## How I worked

The biggest early lesson was memory accounting. Turning transparent huge pages off in step 3 took the peak from 39.0 to 15.47 MB, because the box backs half-used regions with whole 2 MB pages and memory.peak counts all of them. After that I checked memory.peak on every change as well as time, and every push first passed all 320 cases of my gate.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 14.44 | 367 | 0 |
| 1 | Rust v1: a quote-parity pre-pass splits the file for 12 threads, each runs a byte state machine, and one lock-free name table keeps a country bitset per name | run-7bddfb5dd3ec | 0.1204 | 39.3 | 57.4 |
| 2 | Each distinct name copied into one arena entry next to its country bitset | run-edd394160002 | 0.1133 | 39.0 | 58.0 |
| 3 | Transparent huge pages off | run-fbfe0ea92fbf | 0.1190 | 15.47 | 61.5 |
| 4 | Distinct names counted by the workers, from the atomic OR's return value | run-e05eb5fa2929 | 0.1101 | 15.58 | 62.4 |
| 5 | `regions.csv` parsed in one pass, each row's end found by the parse itself | run-e703239bd822 | 0.1016 | 15.31 | 63.2 |
| 6 | Names and notes scanned 32 bytes at a time for their next quote | run-599ba20d6eb1 | 0.0920 | 15.5 | 64.2 |
| 7 | Each name's hash slot loaded as soon as the name is scanned, so its cache miss overlaps the rest of the record | run-7b03d961ed16 | 0.0799 | 15.3 | 65.7 |
| 8 | Numbers parsed from one 8-byte word each, and the number and text-field parsers inlined | run-c66327de718b | 0.0734 | 15.2 | 66.6 |
| 9 | Each distinct name owned by one thread: names travel to their owner in rounds, and each owner marks them in a private table. Region table at one byte per id | run-b8279a47babc | 0.0510 | 14.83 | 69.7 |
| 10 | Each run's first record guessed instead of found by a quote pre-pass, with a check after the parse and an exact rerun when a guess is wrong. Each worker gives its input pages back after the last round | run-b480cbf26ce8 | 0.0418 | 14.8 | 71.7 |
| 11 | Hash slots tagged with 8 bits of the name's hash, at half the size; `regions.csv` parsed on all threads; long names kept in the same table | run-cddb49a6dc40 | 0.0422 | 12.3 | 72.4 |
| 12 | The BOM test reads one byte before it compares three, which takes a `bcmp` call off every record. A sentinel record after the mapped input replaces step 10's exact tail, and one thread replaces the quote pre-pass after a wrong guess | run-213d735f8f1d | 0.0368 | 12.2 | 74.3 |
| 13 | 18 fewer lines in the region table, the exchange and the name set, at the same speed | run-5fd07fa31be9 | 0.0415 | 13.0 | 72.8 |
| 14 | 11 fewer lines: one helper runs both thread groups, and the exchange wakes every worker | run-701b81269ca6 | 0.0368 | 12.4 | 74.3 |
| 15 | The input padded with 32 zero bytes after the sentinel record, so the field parsers lose their end-of-input paths and a per-record `memcpy`; 11 fewer lines | run-94c175e1147b | 0.0427 | 12.6 | 72.7 |
| 16 | Records parsed four at a time: each one's byte in the region table prefetched as soon as its region id is parsed, and the four groups looked up after the fourth record | run-521d52addb7b | 0.0362 | 12.7 | 74.4 |
| 17 | A static-pie build, which skips the dynamic loader | run-521d52addb7b | 0.0362 | 12.7 | 74.4 |
| 18 | A leaner rewrite, 211 fewer lines at the same speed: run starts found exactly from spec §2.2, so the guessed starts, their check and rerun, and the sentinel record go | run-5cc15fc89abe | 0.0351 | 12.4 | 76.5 |
| 19 | 5 fewer lines with the same machine code: three pairs of variables set in one statement each, the regions header skipped in one line, and the cast lint exceptions in `Cargo.toml` | run-ad4d88cb83d7 | 0.0370 | 12.4 | 76.0 |
| 20 | 2 fewer lines with the same machine code: a worker's position, run end and groups set in one statement, and so are a quoted field's start and end | run-66de751504ed | 0.0389 | 13.0 | 75.3 |
| 21 | 3 fewer lines with the same machine code: one `unsafe` statement per `mmap` call, and the digit loop's two updates in one assignment | run-af528e5f49cc | 0.0353 | 12.3 | 76.6 |
| 22 | 1 fewer line with the same machine code: the name set's probe names the hash once and takes the slot and the tag from it | run-af528e5f49cc | 0.0353 | 12.3 | 76.6 |
| 23 | 17 fewer lines outside the record loop: each run's pages given back in two lines, rounded out to whole pages; the region rows cut, marked and written in one line each; and the rounds counted and the parts merged in one line each | run-d4936b9f2b5b | 0.0349 | 12.7 | 76.7 |
| 24 | 1 fewer line: `main` maps both input files through one closure | run-b455f587cb61 | 0.0358 | 12.9 | 76.4 |
| 25 | A name's letter masked to its length by a 24-byte window of a constant, and each letter posted at a running index into its owner's mailbox | run-b89d1be25a5e | 0.0348 | 12.5 | 76.9 |
| 26 | The long-name compare out of line, so the probe for every other name stops paying for its bounds checks | run-8378d5c7581d | 0.0334 | 12.6 | 77.2 |
| 27 | A plain static executable instead of a static PIE (`relocation-model=static`) | run-a8bc41aa5aa0 | 0.0340 | 12.5 | 77.1 |
| 28 | A record's name and note take their quotes from one 64-byte mask, built by AVX2 compares at the name's start, where each quote cost a 32-byte scan | run-4dc3f93575f3 | 0.0319 | 12.5 | 77.7 |
| 29 | `NameSet::read` takes each letter with plain loads and its group as a byte, and `insert` is cold, so marking a name runs 17 fewer instructions per record | run-c02fb16de73f | 0.0312 | 12.5 | 77.8 |
| 30 | 5 fewer lines, three of them with the same machine code: the name set's insert builds its entry in the push and its marking loop takes the group and the key in one statement, `main` writes the output in one line and reads each directory with `nth`, and `merge` sets the min and the max in one assignment | run-3ce10275f327 | 0.0310 | 12.2 | 78.1 |
| 31 | 1 fewer line with the same binary: the Dockerfile's build stage mounts its sources instead of copying them | run-66c99a1d91aa | 0.0309 | 11.9 | 78.2 |
| 32 | The name set's probe ends at its entry's line, so a hit's index is checked once, and the test for a long name moves into the cold compare | run-4ca7cce6e305 | 0.0303 | 11.9 | 78.4 |
| 33 | The join's threads build the region table themselves, in two steps behind the round barrier, instead of on a pool of its own followed by two serial passes, with its pages mapped in on one CPU first; and the name sets are never freed | run-45ab2da93bdb | 0.0291 | 12.2 | 78.7 |
| 34 | The amount parses as one number in cents, every number finds its stop as its first byte below `0`, the BOM test reads the id's word, a letter's stores read `sent[to]` once, the batch is made once per worker, and the input is prefetched 448 bytes ahead | run-91b80b654f7a | 0.0269 | 12.2 | 79.4 |
| 35 | The region id and the amount parse together in one SSE vector, the name and the note come from one mask of quotes and commas whose prefix XOR marks the quoted bytes, the name hash is CRC32, the name set's key stays in one AVX2 register, and a letter's slot is read unchecked | run-6252826712db | 0.0252 | 12.9 | 79.3 |
| 36 | The record batch holds each letter by reference, in an order whose loads are forwarded from their stores, and the group goes into its letter as one byte | run-5a15a73439c1 | 0.0246 | 12.5 | 79.7 |
| 37 | The merge takes its group from `by_id` without a bounds check, and a letter's thread is picked with `sent.len()` instead of a load through the closure's reference to the thread count | run-0ff0124ce494 | 0.0237 | 12.5 | 80.1 |
| 38 | The last worker to finish merges the groups, writes the output and ends the process while the others sleep, so no thread ends first, and each worker gives back its name set's entries after its run; and `main` is C's, so the Rust runtime's setup, whose stack-overflow handler gave every thread a signal stack, does not run | run-490c7c238e52 | 0.0235 | 12.1 | 80.3 |
| 39 | The name's 24 bytes, the region id's and the amount's 16 and the byte after the cents are read without bounds checks, a name set's slot holds its entry above its tag, the quotes are counted with `bzhi`, a CR is told from an LF by its lowest bit, the region id's length takes one compare, and the marking takes its group unchecked | run-d0d0e233e6f7 | 0.0228 | 12.5 | 80.5 |
| 40 | `number` reads a field's word without a bounds check, as an `unsafe fn` whose six calls each show that the 8 bytes lie in the input or its pad, and the BOM test reads its byte unchecked | run-d1b2c0ca269e | 0.0240 | 12.2 | 80.1 |
| 41 | A name is known by a 128-bit hash of its bytes, three AES rounds: a letter is 16 bytes instead of 32 and is posted with one store, and a name set's entry is 40 bytes instead of 64, 64 bits of the hash beside the name's group bits | run-65f94ca68a16 | 0.0226 | 11.5 | 81.1 |
| 42 | 13 fewer lines at the same speed: `region_amount` and `ALIGN` written shorter, the cold paths without `#[inline(never)]`, the mailboxes read in one `for_each` and published by their ends, and `parallel` written into `join` | run-d7d23764cf12 | 0.0219 | 11.4 | 81.6 |
| 43 | A waiter at the barrier sleeps in steps of 10 µs with 1 ns of timer slack, so a sleeper is no longer late for its next round, and rounds of 128 KiB, half as many barriers | run-f276c02c1223 | 0.0219 | 11.4 | 81.6 |
| 44 | 3 fewer lines at the same speed: the country numbers given by one `for_each` over `present().zip(1..)`, with the count from `present().count()`, and the mailbox ends published in one `for_each` | run-42fe84676c85 | 0.0218 | 11.6 | 81.6 |
| 45 | The two hyperthreads of a core mark and parse side by side: a worker on an even CPU marks the round before's letters before its parse and one on an odd CPU after it. The 64-byte window of `text_fields` and the name set's probe read unchecked, and a new min or max is a branch instead of two stores per record | run-15ada601c130 | 0.0206 | 11.4 | 82.2 |
| 46 | `NameSet::read` leaves its loop at a new name, and `insert` calls it again from that letter, so the loop keeps the set's slots, mask and entries in registers instead of loading them for every letter; and `number` parses on a 16-byte SSE window, with the shuffle and multiply-adds of `region_amount` | run-9d22ed670100 | 0.0208 | 11.6 | 82.0 |
| 47 | `NameSet::read` reads a name's home slot and the next as one word and takes the next, with a `cmov`, when the home slot's tag is not the name's, so a name one slot past home no longer mispredicts; only names further on take the probe loop | run-ef37700846c7 | 0.0202 | 11.5 | 82.3 |
| 48 | The region table in one pass: a code's group is the next number when its first row is read, given under a lock, so the second parse of the rows, its scans of the codes and one barrier go, and the output walks the codes in bytewise order to their groups | run-bb0b48a41b0b | 0.0217 | 11.3 | 81.6 |
| 49 | 23 fewer lines at the same speed: the region rows parsed in the worker, with their shares cut by the same closure as the runs; `mallopt` dropped, since no large buffer is freed any more; `map_file` panics with the path; the slow paths of `number`, `text_fields` and the rehash, `values`, `prefix_xor` and the output line written shorter | run-606261c589b8 | 0.0200 | 11.6 | 82.5 |
| 50 | Each worker's 256 groups built on the heap, where `Box::new` built their 16 KiB on the stack first and the stack's pages stayed mapped, and `main` runs worker 0 itself once it has started the others | run-606261c589b8 | 0.0200 | 11.6 | 82.5 |
| 51 | One worker fewer than the CPUs, so a CPU taken by another program leaves a spare one instead of halving a worker and every round | run-d27954c1c3a9 | 0.0217 | 10.7 | 82.0 |
| 52 | 8 fewer lines at the same speed: the Dockerfile's build folded into one `RUN`, the imports in two lines, both `prctl` calls in one, `BOM` and `SPINS` written where they are used, two pairs of variables set in one statement each, and the name's range returned straight from `text_fields` | run-486577623ea1 | 0.0212 | 10.5 | 82.4 |
| 53 | A worker on every CPU again, as in step 50: final scoring runs each batch with nothing else on the box, so step 51's spare CPU only cost time | run-f9622a221638 | 0.0195 | 11.2 | 83.0 |
| 54 | 9 fewer lines at the same speed: the name set's probe tests a hit in one closure and takes its tag and group as bytes of the hash, a long name is told by a `let`-`else`, the slow quote scan carries its parity into bit 0, the name's quote counts are not named, the group's byte is written through its word, the mailboxes are leaked, and `main` reads the data directory where it maps the files | run-dc3871163ade | 0.0194 | 11.4 | 83.1 |

Local numbers are for full seed 1. "P-cores" means CPUs 0 to 11 with all 12 threads and turbo off,
which matches the box within about 7%. "E-cores" means CPUs 12 to 15. They have no hyperthreads, so
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
- Board: 113.3, 118.5 and 119.1 ms over three runs of this code, against 120.4 ms for v1. A
  single board run varies by about ±3%.

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
37.1 to 37.8 MiB, so 22 MiB existed only for moments inside the worker phase. `/proc/self/smaps` put
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

The board measured 15.47 MB, down from 39.0 MB. The 4 KB pages cost about 2% more CPU time and
0.6% more wall time. S 412 → 415.

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

On the board this code measured 110.1 ms in run-e05eb5fa2929 and 111.3 ms in run-a133933027d0.

S 415 → 402, and memory.peak is unchanged at 14.8 to 15.4 MiB. Because the old value is now used,
the OR compiles to a `lock cmpxchg` loop instead of `lock or`. LLVM can emit `lock bts` for this
pattern, but only if the optimizer has not already rewritten the bit test, which it had here. The
loop did not show in task-clock.

## Step 5: regions parsed in one pass

Profiled alone on a P-core, the old parse of `regions.csv` (200k rows, 2.2 MB) took 10.1 to 10.5 ms,
all before any other thread starts. It made two passes, and each one split the rows at LF
(1.4 to 1.6 ms) and then parsed each row (3.5 to 4 ms), with a search for the comma, a validation loop
per field and a fold per field.

Region ids ascend (spec §2.3), so the last row's id now sizes the array up front and one pass is
enough. That pass stores each region's country code and marks the countries present, and once the
countries are numbered, one walk over the array swaps each code for its group. A row parses as
digits up to the comma, followed by a fixed `,XYZ` and LF that one slice pattern checks, so the
row's end comes out of the parse without a separate search. Alone, the new parse takes 2.9 to 3.1 ms:
2.5 to 2.7 ms for the rows, 0.5 to 0.6 ms for the walk over the 1M-entry array and 0.03 ms to number the
countries.

P-cores, `perf stat -r 10`, three interleaved rounds:

| | step 4 | step 5 |
|---|---|---|
| task-clock (ms) | 1144 / 1149 / 1164 | 1144 / 1150 / 1145 |
| elapsed (ms) | 116.8 / 113.1 / 114.1 | 105.6 / 106.7 / 108.3 |

S 402 → 416, and memory.peak is unchanged at 15.0 to 15.1 MiB. The parse is now stricter outside
the spec: a blank line, a last row without its LF, or an id larger than the last row's returns an
error, where the old parse accepted them.

## Step 6: quoted fields scanned 32 bytes at a time

`perf record` on P-cores put 76% of the samples in the worker loop, which has the name table
inlined into it, 17% in `text_field`, 4.6% in the quote pre-pass and 0.2% in the regions parse.
Knockouts of step 5 then split the worker loop (P-cores, `perf stat`, each knockout beside its
baseline in the same session):

- Without the name table a run takes 557 ms of task-clock and 58 to 59 ms elapsed, against
  1129 to 1142 ms and 106 to 118 ms. Without the region lookup as well, 553 ms and 56 to 57 ms. The scan
  is half the time, the name table the other half, and the region lookup about 2%.
- Hashing each name and loading its slot costs 0.1 G cycles: 1.39 G against 1.29 G for the scan
  alone. Finding the arena entry, comparing the name and setting the bit cost 0.92 G more when
  the slot is loaded right after the name. The entry adds 2.7 L2 misses per record and 3.8 M page
  walks, which are active in 7% of all cycles, since the arena has had 4 KB pages since step 3.
- The atomic OR costs 7% of task-clock: 1059 to 1089 ms when the bitset word is only read, against
  1146 to 1166 ms with the OR. It also moves lines between cores: 1.37 M loads are served from
  another core's cache, against 0.28 M without it. Testing the bit before the OR (1172 to 1199 ms)
  saves nothing, because 90% of records set a new bit.

Names and notes are 37 of a record's 60 bytes on average, and the old `text_field` stepped
through them one byte at a time, with a compare and a branch per byte. Inside a quoted field only
the quotes matter, so `text_field` now compares 32 bytes with `"` at once and jumps to the first
match, the lowest set bit of the 32-bit mask. A name or a note usually ends within the first
step. The mask is built by a plain loop over a `[u8; 32]`, which compiles to one `vpcmpeqb` and
one `vpmovmskb`, so the scan needs no `unsafe` and no intrinsics. Instructions fall from 2.585 G
to 2.035 G.

P-cores, `perf stat -r 10`, three interleaved rounds:

| | step 5 | step 6 |
|---|---|---|
| task-clock (ms) | 1152 / 1143 / 1136 | 1021 / 1002 / 985 |
| cycles:u (G) | 2.58 / 2.55 / 2.54 | 2.27 / 2.22 / 2.17 |
| elapsed (ms) | 110.6 / 105.6 / 108.9 | 94.7 / 95.2 / 93.1 |

S 416 → 426, and memory.peak is unchanged, 14.7 to 15.0 MiB. On the board this code measured
92.0 ms in run-599ba20d6eb1, at commit 2f88e04, which carries the same track C code.

## Step 7: each name's slot loaded as soon as the name is scanned

Step 6's knockouts found that hashing a name and loading its slot cost only 0.1 G cycles when
the slot load sat right after the name. In step 6 it sat at the end of the record, inside
`mark`, after the note and the numbers, so each record stalled on the slot's cache miss with
nothing left to parse. Now `NameTable::probe` hashes the name and loads its home slot as soon as
`text_field` returns the name, and `mark` takes the hash and the slot's value at the end of the
record. The note and the three numbers parse while the miss is pending.

The early value can only be out of date in one way. A slot that holds a name never changes, and
an empty slot that another thread fills in between makes `intern` lose the race to publish, a
case it already handled. A full run still interns about 100.53 k names and wastes at most about
15 entries more than before. A build whose probe always reports an empty slot, which sends every
record down that path, passes the gate and 400 random cases against the reference.

P-cores, `perf stat -r 10`, three interleaved rounds:

| | step 6 | step 7 |
|---|---|---|
| task-clock (ms) | 1005 / 1032 / 1017 | 912 / 933 / 926 |
| cycles:u (G) | 2.22 / 2.29 / 2.25 | 2.00 / 2.05 / 2.03 |
| elapsed (ms) | 97.8 / 99.2 / 99.7 | 89.6 / 91.1 / 87.4 |

Instructions barely move, 2.035 → 2.046 G. S 426 → 432, and memory.peak in the container with
the harness's flags is unchanged, 14.8 to 15.0 MiB. On the board this code measured 79.9 ms in
run-7b03d961ed16, at commit ce4de64, which carries the same track C code. The board skipped
b9cb846 itself when a newer push replaced it in its queue.

## Step 8: numbers parsed from one word, and the field parsers inlined

A line-level profile of step 7 (P-cores, `perf record`) still puts most of the worker loop in the
name table. On the parsing side, the loops over number digits held about 10% of the samples, and
3.1% sat right after a 16-byte load that read back part of `text_field`'s result from the
stack. Branch misses ran at 2.7 per record, 9.06 M in all. Three changes followed, each measured
on top of the one before (P-cores, three interleaved rounds):

1. `number` parses a field whose stop byte lies within 8 bytes from one little-endian word. XOR
   with the stop byte in every byte makes each stop a zero byte, and the zero-byte test finds
   the first one. Shifting the digits to the top of the word turns the missing ones into leading
   zeros, and three multiplies join digits into pairs, pairs into fours and fours into the
   value. There is no loop and no branch on the field's length. Longer fields, which §2.1 allows
   with leading zeros, and the file's last 7 bytes keep the byte loop. Measured with
   `perf stat -r 10`: task-clock 916 / 899 / 911 → 859 / 864 / 871 ms, cycles:u
   2.01 / 1.97 / 2.00 → 1.87 / 1.89 / 1.91 G, instructions 2.046 → 1.872 G, and branch misses
   9.06 → 5.03 M.
2. `text_field` is always inlined. As a call it returned four words through the stack, and the
   caller read two of them back with one 16-byte load. Store forwarding cannot merge two 8-byte
   stores into one load, so that load waited until both stores reached the cache. Measured with
   `perf stat -r 10`: task-clock 876 / 883 / 868 → 841 / 840 / 842 ms, cycles:u
   1.92 / 1.94 / 1.90 → 1.84 / 1.83 / 1.84 G, and instructions 1.872 → 1.769 G.
3. `number` is always inlined too. At each call site `stop` is a constant, so the call and the
   multiply that copies `stop` into every byte disappear, 29 instructions per record. Measured
   with `perf stat -r 30`: task-clock 844 / 839 / 845 → 824 / 812 / 824 ms, elapsed
   80.7 / 79.6 / 81.5 → 78.6 / 78.3 / 82.3 ms, and instructions 1.769 → 1.673 G.

A plain `#[inline]` left `text_field` a call, so both use `#[inline(always)]`. `number` was
checked against the byte loop on 200,040 fields: every length from 0 to 20 digits, both stops,
random bytes after the stop, and 20-digit values that wrap.

Step 7 → step 8 on P-cores, three interleaved rounds:

| | step 7 | step 8 |
|---|---|---|
| task-clock, `perf stat -r 30` (ms) | 901 / 911 / 907 | 830 / 824 / 828 |
| elapsed, `perf stat -r 30` (ms) | 87.5 / 89.0 / 87.8 | 78.8 / 78.5 / 79.4 |
| cycles:u, `perf stat -r 10` (G) | 2.05 / 1.98 / 2.03 | 1.82 / 1.81 / 1.80 |
| instructions:u (G) | 2.046 | 1.673 |
| branch misses (M) | 9.06 | 5.10 |

S 432 → 447: 13 lines for the word parse and 2 for the attributes. memory.peak in the container
with the harness's flags is unchanged, 14.7 to 14.8 MiB against 14.8 to 15.2 MiB for step 7.

## Step 9: each distinct name owned by one thread

Steps 2 to 7 tuned one name table that all threads shared, and it still cost about half the
time. With all name hashing, posting and marking knocked out of the prototype below, the rest ran
in 42.6 ms (local, median of 11), against 85 to 88 ms for step 7 in the sessions around it. In
step 7 each record's lookup missed the cache on a line that other cores also wrote, and set its
bit with a `lock cmpxchg` loop.

Now each name has one owner thread, picked by its hash, and only the owner touches the name's
entry. The workers run in rounds. In each round a worker parses 64 KiB of its chunk and posts
each record's name and group to the owner's mailbox as a 32-byte letter, and then all workers
meet at a barrier. After it, each worker marks the letters posted to it in its own hash table.

- A name of up to 24 bytes travels inside its letter, zero-padded, with its hash and length in
  the fourth word, so the owner compares 32-byte keys and never reads the file. Longer names
  travel as offsets into the file and go to a `HashMap`. They are rare: 203 of seed 1's 3.32 M
  records.
- An entry is one 64-byte line: the 4-word key, then the 241-bit country bitset. The compare and
  the bit share the line, and the bit is set with a plain OR.
- The mailboxes come in two sets that alternate by round, so one barrier per round separates
  posting from reading. A waiting worker spins 2000 times and then parks, and the last worker to
  arrive unparks the others.
- The region table holds one byte per id, 1 MB instead of 2 MB. Groups past 254, which the
  grammar allows and the generator never writes, read as 255 and are looked up in a sorted side
  list.
- `mallopt(M_MMAP_THRESHOLD, 64 KiB)` runs at startup, and each table reserves room for twice
  its expected names (pages that are never written are never charged). A build without either
  peaked at 19.8 to 20.2 MiB. Freeing the regions parse's 2 MB scratch array had raised glibc's
  dynamic mmap threshold to 2 MB, so the tables came from malloc arenas and grew there by
  copying, and the old copies stayed resident.

The prototype was built on step 7, before step 8 existed, in stages (local, P-cores, medians of
15 interleaved runs against the build before it unless noted):

1. Names sent as offsets into the file and compared there, with a barrier that only spins:
   69.6 ms against 88.0 ms for step 7, but with runs of 161 and 171 ms. When the scheduler
   delays one worker, the other eleven keep their CPUs busy spinning.
2. A barrier that yields instead: 69.8 against 85.0 ms, with runs of 90, 129 and 147 ms.
3. Spinning 2000 times and then parking: 70.2 against 86.1 ms, worst run 77 ms. Kept.
4. Names of up to 24 bytes inside the letter, and entries of one cache line: 64.2 against
   70.3 ms.
5. The marking loop kept out of line (63.4 against 64.0 ms, 11 runs), then the byte-wide
   region table, `mallopt` and the doubled reserve: 59.4 against 63.9 ms.

Knockouts of stage 3 (medians of 11, one session): 71.5 ms in full, 51.6 ms with the letters
posted and read but no name marked, and 42.6 ms with no name hashed, posted or marked (the rounds
and barriers stay). So marking cost about
20 ms and the exchange about 9 ms, and stage 4 went after the marking. `perf stat -r 5`:

| | step 7 | stage 3 | stage 4 | no name work |
|---|---|---|---|---|
| task-clock (ms) | 891 | 727 | 620 | 390 |
| cycles:u (G) | 1.949 | 1.669 | 1.403 | 0.890 |
| instructions:u (G) | 2.046 | 2.257 | 2.094 | 1.379 |
| branch misses (M) | 9.06 | 12.75 | 6.21 | 5.68 |
| loads that missed L2 (M) | 12.1 | 9.08 | 6.44 | 2.89 |
| elapsed (ms) | 87.7 | 71.8 | 64.2 | 42.6 |

Phases of stage 4, from an instrumented copy that logged timestamps per thread: each worker
spends 31.5 to 32.1 ms parsing and posting, 4.5 to 5.5 ms waiting at 257 barriers (parking 18 to 21
times), and 10.2 to 11.0 ms marking. A wait takes 4 to 6 µs at the median, 13 to 21 µs at p90 and
330 to 380 µs at p99. Of the marking, reading the letters takes 2.44 ms per worker and finding
their slots 0.09 ms more (2.53 ms in all), against 10.79 ms in full. The entries cost 69
instructions, 87 cycles and 1.01 L2 misses per letter on top of reading it.

Step 9 is that prototype on top of step 8. P-cores, three interleaved rounds:

| | step 8 | step 9 |
|---|---|---|
| task-clock, `perf stat -r 10` (ms) | 837 / 841 / 829 | 498 / 498 / 497 |
| elapsed, `perf stat -r 10` (ms) | 81.6 / 79.2 / 81.9 | 52.3 / 52.3 / 51.7 |
| cycles:u (G) | 1.83 / 1.84 / 1.80 | 1.12 / 1.12 / 1.12 |
| instructions:u (G) | 1.673 | 1.659 |
| branch misses (M) | 5.09 | 2.90 |
| memory.peak, user scope (MiB) | 14.42 / 14.56 / 14.29 / 14.76 | 13.90 / 13.77 / 14.18 / 13.96 |

Over 21 interleaved runs, the wall-time medians are 80.71 ms for step 8 (min 75.40) and 52.77 ms
for step 9 (min 51.65), with the stage-5 prototype on step 7 at 59.54 ms in the same session.
With the private tables, THP no longer changes the peak: 13.80 to 14.18 MiB with the `prctl` and
13.76 to 14.28 MiB without it (4 runs each), so the call stays only as insurance. S 447 → 587.

Correctness checks: the gate (307 pass, 0 fail); fresh seeds 5 to 7; full seeds 2 and 3 at 1 and
64 threads; and the 400 random cases plus two long-name sets against the reference, at 1, 3,
12, 37 and 64 threads, and again with 100-byte rounds and 3-letter mailboxes at 1, 2, 3, 12
and 64 threads, which forces thousands of rounds and early round ends. The long-name sets hold
every name length from 0 to 80 bytes, names equal in their first 24 bytes, trailing-space
twins, `""` escapes at the 8-byte word edges, and quoted and unquoted spellings of one value.

## Step 10: guessed run starts, and the input released by the workers

This step goes after the time outside the worker loop. A timeline of step 8 (P-cores, an
instrumented copy) put the regions parse at 3.1 to 3.3 ms, the quote pre-pass at 6.4 to 6.7 ms, the
workers at 62 to 68 ms, the merge at 0.7 to 1.0 ms and the write at 0.05 ms. When the process
exits, the kernel unmaps the 200 MB input on one thread: an explicit `munmap` took 2.8 ms, and
the time from the program's last timestamp to the end of the wall was 4.5 to 5.1 ms (medians of
40 runs, two sessions).

1. **Each worker releases its own run.** After the last round, every worker calls
   `madvise(MADV_DONTNEED)` on the whole pages of its run, so the unmapping runs on twelve
   threads instead of one at exit. It waits for the last round because dropping pages makes
   the kernel flush every thread's TLB, which slows any worker still parsing. On step 8, the
   gap after the last timestamp fell from 4.51 and 5.08 ms to 2.03 and 2.70 ms (medians of 40
   runs, two sessions). Releasing without waiting for the other workers was slower there:
   `perf stat -r 30` elapsed 80.3 / 81.9 / 80.0 ms, against 79.6 / 77.8 / 79.3 ms after a
   barrier. Step 9's rounds already end at a barrier, so the release needs none of its own.
2. **Run starts are guessed.** Instead of the pre-pass, each cut takes the first LF after it
   that a digit or a BOM follows. A record starts that way, and §2.2 says a note's LF is always
   followed by a lowercase word or a quote, so on generated data no guess is wrong. §2.1 allows
   any byte there, so the guesses are checked: each worker returns where the parse of its run
   stopped, and every run must end exactly where the next one starts. Otherwise the join runs
   again on the pre-pass's exact cuts, with a fresh exchange. The parse is exact from the file's
   start, so if each run ends on the next one's start, by induction every start is a record
   start.

   A wrong guess must cost time, never a crash, and a misread record can read past its run.
   Every scan in a record stops at a comma, a dot or an odd run of quotes, and a quoted field
   that holds more than quotes opens and closes with an odd run. So a misread record never
   reads past the first real record whose name and note are such fields. The file's tail, from
   the first record start in its last 4 KiB, has an exact start: a valid file holds an even
   number of quotes, so the parity of the quotes in the last 4 KiB is the quote state where
   they begin. The tail must hold such a record, which takes a few records' parse to find, or
   the join uses the pre-pass. The guessed runs all end at the tail's start, and the last worker parses the tail
   after its run, from that exact start. A separate thread for the tail would have shared a CPU
   with a worker and waited at all of the ~257 barriers.

Both on top of step 9, P-cores, wall time of interleaved runs (`ab.py`), median and min in ms:

| | step 9 | release only | guesses only | both |
|---|---|---|---|---|
| session 1, 60 runs each | 52.68 / 51.09 | 51.30 / 49.99 | 47.69 / 46.74 | 46.43 / 45.50 |
| session 2, 60 runs each, order reversed | 53.61 / 51.49 | 52.25 / 49.96 | 48.88 / 46.51 | 47.81 / 45.48 |
| session 3, 60 runs each | 54.04 / 51.19 | | | 47.88 / 45.73 |

The release saves 1.1 to 1.5 ms and the guesses 4.4 to 5.0 ms; together 5.5 to 6.3 ms. In the quietest
`perf stat -r 10` round, task-clock was 501.4 ms for step 9, 511.3 with the release, 490.4 with
the guesses and 501.6 with both. Page faults stay at 6,675: the workers now fault the input in
as they parse it.

memory.peak in the container with the harness's flags and 12 CPUs, 12 alternating runs each:
13.3 to 14.3 MiB (mean 13.99) for step 9 and 13.9 to 14.6 MiB (mean 14.16) for step 10. S 587 → 630:
10 lines for the release and 33 for the guesses, their check and the tail.

Correctness checks:

- The gate (307 pass, 0 fail). Every guess held on all 300 tiny seeds at 1, 3, 12, 37 and 64
  threads, and on full seeds 1 to 7 at 1, 12 and 64 threads.
- The 400 random cases, and 400 hostile ones against the reference at 1, 3, 12 and 64 threads
  with 64-, 200- and 1,000-byte tails. The hostile cases put into names and notes what §2.1
  allows and §2.2 rules out: LFs followed by digits or BOMs, whole fake records, CRLFs, dots and
  stray quotes. Up to 332 of the 400 runs guessed wrong, and all reran to the reference's output.
- A forced test on every byte offset of 7 tiny seeds, the worked example, 4 random cases and 60
  hostile ones, with 64-, 1,024- and 4,096-byte tails: 648,626 exact cuts gave the same output
  and ended where the next run starts, and 498,540 offsets forced as guesses never crashed, were
  accepted exactly when they were record starts, and then gave the same output.

## Step 11: tagged hash slots, and regions parsed on all threads

1. **Tagged slots.** Each u32 slot of a worker's name table holds its entry's index plus one in
   the low 24 bits and 8 bits of the name's hash in the top 8. A probe that meets another name's
   slot sees the tag differ and moves on without loading that name's entry line. With the tags
   the table halves to 16k slots per worker, sized at 4/3 of the names it will hold and grown
   at 3/4 load. Halving the slots without tags cost 3% of T; with them T fell 4.8%.
2. **`regions.csv` parsed on all threads**, one even share of the rows each, into row lists
   sized up front. The serial parse and the walk over the 1M-entry id array took 3.8 ms before
   the join could start; the parallel parse takes about 2.5 ms. Without the sizing, glibc's
   per-thread arenas gave 1.5 MiB of the saving back.
3. **Fewer lines.** Names over 24 bytes go into the same tagged table. Their key is a pair of
   file offsets, compared in the file only when the hashes match, so the separate `HashMap`
   goes. Groups wider than 254 countries use a `HashMap` instead of a sorted list. Region ids no
   longer need to ascend: the reference accepts them in any order, and step 10 stopped with an
   error on them.

On top of step 10, P-cores, wall time of interleaved runs (`ab.py`), median / min in ms:

| | step 10 | step 11 |
|---|---|---|
| session 1, 41 runs each, load average 18 | 47.96 / 45.70 | 44.16 / 42.08 |
| session 2, quieter | 46.44 / 45.60 | 43.17 / 42.58 |

memory.peak in the container with the harness's flags, 8 runs each: median 13.84 MiB
(13.35 to 14.01) for step 10 and 12.39 MiB (11.78 to 12.73) for step 11. S 630 → 622.

Correctness checks: the gate (307 pass, 0 fail); the 402 random cases at the default thread
count and at 1, 3, 12, 37 and 64 threads; tiny rounds (100-byte rounds, 3-letter mailboxes) at
1, 2, 3, 12 and 64 threads; and against the reference, at the default count and at 64 threads,
a region set with groups wider than 254 countries and one with shuffled ids. A bad region row
still stops the join with its error.

## Step 12: one `bcmp` call less per record, and 59 lines less

1. **The BOM test.** `data[at..].starts_with(BOM)` compiled to a call to glibc's `bcmp` for
   every record, after a `vzeroupper`, because LLVM left the 3-byte compare as a call. Testing
   the first byte first leaves the call to the records that start with 0xEF.
2. **A sentinel record instead of the exact tail.** `map_file` maps `records.csv` over a
   reservation of anonymous memory and writes one more record after it, `0,"a","a",0,0.00` on a
   line of its own. Its name and note are quoted, so a parse from a wrong guess stops within it
   at the latest, as step 10 showed for any such record. Step 10's tail is then redundant: no
   quote parity over the last 4 KiB, no search there for such a record, no jump of the last
   worker into the tail, and no `next_record_start`. The last run ends at the end of the file.
   The mapping is writable and private; writing the sentinel copies at most the file's last
   page, and no run's pages hold it, so the release never drops it.
3. **One thread instead of the pre-pass after a wrong guess.** Guesses go wrong only on files
   outside §2.2, where speed is not scored, so the rerun parses the whole body on one worker
   from its exact start, and `record_bounds` and its parallel quote count go.
4. **Smaller cuts.** `Group::EMPTY` replaces the `Default` impl, and the output walk zips the
   keys with the groups.

On top of step 11, P-cores, wall time of interleaved runs (`ab.py`, 101 runs each), median / min
in ms. "Lines only" is items 2 to 4 without item 1.

| | step 11 | lines only | step 12 |
|---|---|---|---|
| session 1 | 43.17 / 41.87 | 43.36 / 41.91 | 40.47 / 39.71 |
| session 2 | 43.09 / 41.83 | | 40.57 / 39.68 |

memory.peak in the container with the harness's flags, 6 runs each: 12.7 to 13.3 MB for step 11
and 12.5 to 13.5 MB for step 12. Page faults stay at 6,390. S 622 → 563: the pre-pass -23 lines,
the tail -27 (-40 there, +13 for the mapping and its constants), `Group::EMPTY` -4, the output
walk -5.

Correctness checks: the gate (307 pass, 0 fail); full seeds 1 to 4 against the reference; the
400 random and 400 hostile cases at the default thread count and at 1, 3, 12, 37 and 64 threads,
where up to 397 of the hostile runs guessed wrong and all reran to the reference's output; and
every byte offset of the body of 72 files (490,079 offsets), each forced as an exact cut and as
a guess with the second run reading on into the sentinel: no crash or hang, and the 8,783
offsets that are record starts tiled and gave the same output.

## Step 13: 18 fewer lines

Line cuts in the region table, the exchange and the name set, checked against the machine code:
the parse loop compiles to the same instructions as step 12's; only the barrier's spin loop and
the order of one bit test in `NameSet::read` changed. Three cuts that changed the hot loop's
code were put back: each cost 0.15 to 0.3 ms. `ab.py`, 50 interleaved runs, median / min: step 12
40.51 / 39.68 ms, step 13 40.77 / 39.62 ms. memory.peak, 6 runs: 11.68 MiB median, within the
±0.8 MiB of the per-CPU charge batching. S 563 → 545. Gate: 307 pass, 0 fail; the 402 random
cases at 1, 3, 12, 37 and 64 threads and in the tiny-round build; the output hash is unchanged.

## Step 14: 11 fewer lines

A 6-line `parallel(n, &work)` helper replaces the two `thread::scope` spawn and join blocks, in
the regions parse and the join. `Exchange::wait` now unparks every worker, itself included,
which is harmless and drops its `me` parameter. The record loop is 585 instructions in both
builds; the only difference in the hot code is the instruction order inside the hash of
`wide[&id]`, which runs only for a region group of 255 or more, and 240 countries never get
there. Dropped: removing the barrier leader's early return, which saved a line but changed the
hot loop's register allocation.

`ab.py` against step 13, median / min: full runs (60 each) 40.44 / 39.73 → 40.34 / 39.58 ms;
regions only (100 each) 4.73 / 4.24 → 4.81 / 4.16 ms. memory.peak, 4 runs: 12.23 → 12.22 MiB.
S 545 → 534. Gate: 307 pass, 0 fail; the 402 random cases at 1, 3, 12, 37 and 64 threads and in
the tiny-round build; the output hash is unchanged.

## Step 15: a padded input, and no `memcpy` per record

The board's first run of this code, run-46d1884323c7, drew 54.9 ms while A, B and D ran at
their usual times; the next run of the same code, run-94c175e1147b, took 42.7 ms: that is the
row. Every worker waits at each round's barrier, so one interrupted thread delays them all.

The sentinel record after the file is now followed by 32 zero bytes. Every position a parse
reaches lies before those zeros, so the fixed-size reads of `number`, `text_field` and
`name_letter` always fit, and their paths for the end of the input go. That also removes the
one libc call left on the per-record path: LLVM had merged `name_letter`'s 24-byte copy and its
shorter end-of-input copy into one `memcpy` with a variable length. An audit of the per-record
path finds no other call; the `bcmp` in `NameSet::read` runs only for names over 24 bytes.
Two smaller cuts: `guess_bounds` computes its cut in a closure, and the output loop filters out
countries without records.

Local, on top of step 13 (P-cores, a loaded machine, 151 interleaved runs, median / min):
42.34 / 39.60 → 42.08 / 39.53 ms. `perf stat -r 10`, three interleaved rounds: instructions
1.637 → 1.549 G (-5.4%), cycles and task-clock flat, page faults 6,390 in both. Merged with
step 14, S 534 → 523, and 522 after inlining the single-use `KEY` constant, which leaves the machine code
identical. Gate: 307 pass, 0 fail; full seeds 1 to 7; the 400 random and 400 hostile
cases at 1, 3, 12, 37 and 64 threads; the forced-cut test on 490,079 offsets of 72 files.

## Step 16: the region lookup prefetched, four records at a time

At step 15 the byte load in `Regions::group` was the parse loop's one load that missed L2: 40% of
all L2 misses and 6.5% of all cycle samples (P-cores, `perf record`). The table is 1 MB, which
does not stay in a core's 1.25 MB L2 next to the input and the name sets, so the load goes to L3
(per run, 4.3 M loads hit L3 and 54 K missed it). With the lookup replaced by `id & 127`, a wrong
output, the median wall time fell 4.9%, the most that hiding the load could give.

The worker now parses and posts four records, and prefetches each one's byte in the table as
soon as its region id is parsed. Only after the fourth does it look up the four groups, OR each
group into the last word of its letter, which no one reads before the barrier, and add each
record to its group. A prefetch retires without waiting for its line, so it does not hold up
the reorder buffer the way the load did.

On top of step 15, P-cores, wall time of interleaved runs (`ab.py`, 151 runs each), median /
min: 41.09 / 40.07 → 39.85 / 38.96 ms. `perf stat -r 10`, three interleaved rounds: cycles
0.953 to 0.958 → 0.919 to 0.920 G (-3.7%), task-clock 425 to 427 → 411 to 412 ms, instructions 1.550 →
1.655 G, page faults 6,390 in both. In 101-run sessions, batches of 3 and 4 measured -2.8%, 2
-1.2%, 6 -1.8% and 16 -1.9%, and the batch of 4 without the prefetch +1.3%. S 522 → 534.

Variants that did not stay, against step 15 in the same sessions:
- The prefetch alone, with the lookup where it was: flat (41.05 against 41.07 ms).
- Each record's lookup one record late instead of in batches: -0.8% to -1.1%. The record carried
  past the next parse spilled to the stack, 20 more instructions per record.
- The batch as a `Vec` in the record loop: +1.9%.
- A non-temporal prefetch of the input 1 KiB ahead, to keep it out of L2: +2.2%.
- Rounds of 32 KiB: +3.5%. Rounds of 128 KiB with 512-letter mailboxes: flat, with more memory.
- A rank table for the regions on top of this step: one word per 32 ids with their presence
  bits and the count of the regions before them, and one group byte per region, 450 KB in all.
  It saved 134 page faults, about 0.5 MB, but cost +6.0% wall time and +12% task-clock: the
  second load depends on the first, and the prefetch covers only the first.

Correctness checks: the gate (307 pass, 0 fail); full seeds 1 to 7; the 400 random and 400
hostile cases at the default thread count and at 1, 3, 12, 37 and 64 threads; the same cases on
a build with 100-byte rounds and 3-letter mailboxes, which ends most rounds with a full mailbox
and a partial batch, at 1, 2, 3, 12 and 64 threads; and the forced-cut test on 490,079 offsets
of 72 files.

## Step 17: a static-pie build

`-C target-feature=+crt-static` in `.cargo/config.toml` (free on S) links the binary as a
static PIE, so the process starts without the dynamic loader, as track A has done since its
step 14. `ab.py`, 40 interleaved runs on top of step 16, median / min / mean: 40.14 / 38.97 /
42.65 ms dynamic, 39.70 / 38.86 / 40.73 ms static. Gate: 307 pass, 0 fail; the image built
offline and run with the harness's sandbox flags gives seed 1's hash.

Steps 16 and 17 went to the board in one push, so both rows cite run-521d52addb7b.

## Step 18: a leaner rewrite

Lines were what kept C behind: at S 534 against an Sref of 108, they cost about 1.9 points
next to an entry of the same speed at S 298. So the design was written again with only the
parts that measured as paying. Spec §2.2 makes the run starts exact: inside a note an LF is
followed by a lowercase word or a quote, never by a digit or a BOM, so the first LF after a cut
that a digit or a BOM follows starts a record. That drops the guessed starts, the check that
the runs tile the file, the rerun when they don't, and the sentinel record. The name exchange
keeps its design (owners, 32-byte letters, 64 KiB rounds, one barrier per round) with plain
mailboxes instead of the `Exchange` type. Each mailbox now holds a whole round's worst case,
64 KiB of 11-byte records, so no round ends early, every worker takes part in as many rounds
as the longest run needs, and the barrier only counts arrivals: a waiter spins, then sleeps in
50 µs steps, where step 17's parked until the last worker to arrive unparked it. The field
parsers, the region table and the name sets keep their designs, less step 11's map for groups
past 254 countries: more than 255 countries now stop the join with an error (§2.3 has 240),
and a name's entry is always one 64-byte line. S 534 → 323.

- `ab.py` against step 17, interleaved, medians / minimums: 101 runs twice, 39.82 / 38.86 →
  39.43 / 38.07 and 39.96 / 38.59 → 39.55 / 38.31 ms; 60 runs on seed 1, 39.92 / 38.77 →
  39.26 / 37.92 ms; 60 runs on seed 3, 40.02 / 38.19 → 39.49 / 38.35 ms.
- memory.peak: 60 alternating rounds, medians 11.890 → 11.880 MiB; a 6-round sample in another
  session, 12.17 to 12.94 → 12.27 to 13.86 MB.
- Correctness: the gate, 307 pass, 0 fail; full seeds 1 to 7 give the reference hashes; the 402
  fuzz cases pass at 1, 3, 12, 37 and 64 threads and with 100-byte rounds at 1, 2, 3, 12 and 64;
  the 300 tiny seeds pass at 10 thread counts with 100-byte rounds; the image built offline and
  run with the harness's sandbox flags gives seed 1's hash. Of step 11's region sets, the one
  with shuffled ids passes, and the two with 689 and 692 countries stop with the error.

## Step 19: 5 fewer lines, the same machine code

Line cuts on step 18 that leave `.text` and `.rodata` byte-identical to step 18's binary
(`objcopy` and `cmp`). `batch` and `parsed` are declared together, and so are `counts` and
`arrived`; the step past an escaped quote sets `pairs` and `end` in one assignment;
`parse_regions` finds the body after the header in one line; and the crate's two cast lint
exceptions move to `Cargo.toml`, next to the one already there. Dropped: `names` and `sent`
declared together, which moved one allocation's code. S 323 → 318. Gate: 307 pass, 0 fail;
clippy reports no warnings.

## Step 20: 2 fewer lines, the same machine code

Two more declarations merged on step 19, with `.text` and `.rodata` byte-identical to step 19's
binary: each worker sets its position, its run's end and its groups in one statement, and
`text_field` sets the field's start and its scan position, both just past the opening quote, in
one. Dropped because the machine code changed: the round's `half` and `round_end` declared
together, and `regions.csv` mapped inside the `parse_regions` call, which drops its path later.
S 318 → 316. Gate: 307 pass, 0 fail; clippy reports no warnings.

## Step 21: 3 fewer lines, the same machine code

On step 20, with `.text` and `.rodata` byte-identical to step 19's binary: `map_file` makes its
two `mmap` calls as two statements, each with its own safety comment and its flags named above
it, instead of one `unsafe` block, and the byte loop of `number` updates the value and the
position in one assignment. Dropped because the machine code changed: `merge` setting the min
and the max in one assignment. S 316 → 313. Gate: 307 pass, 0 fail; clippy reports no warnings.

## Step 22: 1 fewer line, the same machine code

`NameSet::read` names the letter's 32-bit hash once and takes the first slot and the tag from
it, where it cast `key[3]` twice, so its three declarations before the probe become two.
`.text` and `.rodata` are byte-identical to step 21's binary. S 313 → 312. Gate: 307 pass,
0 fail; clippy reports no warnings.

Steps 21 and 22 went to the board in one push, so both rows cite run-af528e5f49cc.

## Step 23: 17 fewer lines outside the record loop

Cuts on step 22 outside the record loop. The worker's code differs from step 22's only at its
end and in register choices, a few stack moves and alignment padding:

- Each worker gives its run's pages back in two lines at the end of its closure, instead of
  through `release`. The range now runs from the page that holds the run's first byte to the
  page that holds its last, where `release` kept to the pages wholly inside the run. A
  neighbour that still reads a shared page, at the end of the run before, maps the same bytes
  from the file again, as a long name compared after the release already could. 4 lines.
- `parse_regions` keeps the header's LF at the start of `body`, so a row starts at `p` exactly
  when `body[p - 1]` is an LF, and each cut is one `find` (3 lines). Marking the countries that
  occur along with the largest id, and writing each region's group, take one statement each
  (5 lines), and `CODES` is written out at its one use (1 line).
- In `join`, a run's start is one closure, and the rounds are counted and the parts merged in
  one statement each. 4 lines. `guess` is now `run_start`, since step 18 made the starts exact.

S 312 → 295. `ab.py` against step 22, 101 interleaved runs, medians / minimums, two sessions:
seed 1, 39.28 / 38.21 → 39.07 / 38.24 and 39.25 / 38.01 → 39.16 / 38.17 ms; seed 3,
39.18 / 38.19 → 39.04 / 37.78 and 39.08 / 38.21 → 38.90 / 38.04 ms. memory.peak, 40
alternating rounds: medians 11.950 → 11.930 MiB, means 12.024 → 11.915 MiB. Correctness: the
gate, 307 pass, 0 fail; full seeds 1 to 7 give the reference hashes; the 402 fuzz cases pass at the
default thread count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12
and 64; the 300 tiny seeds pass at 10 thread counts with 100-byte rounds; clippy reports no
warnings.

## Step 24: 1 fewer line

`main` maps both input files through one closure that takes the file's name. Only `main`'s code
changes, with `map_file` now inlined into the closure, and `NameSet::read` moves 96 bytes down
the binary, onto a 32-byte boundary instead of a 64-byte one. S 295 → 294. `ab.py` against
step 23, interleaved, medians / minimums: 101 runs on seed 1, 39.30 / 38.16 → 39.31 / 38.34 ms;
101 runs on seed 2, 39.24 / 37.95 → 39.21 / 38.06 ms; 61 runs on seed 3, 39.08 / 38.17 →
39.09 / 38.15 ms. Correctness: the gate, 307 pass, 0 fail; full seeds 1 to 7 give the reference
hashes; the fuzz cases, the tiny seeds and the region sets give step 23's results; a missing
input directory still stops with the file's path in the error.

## Step 25: fewer instructions to build and post a letter

Two cuts in the record loop on step 24:

- `name_letter` keeps the first `len` bytes of a name by ANDing its three words with a 24-byte
  window of `KEEP`, 24 bytes of ones and then 24 of zeros, taken from `24 - len` on. Each word
  was masked by a shift of a `u128` by its own byte count, which took a clamp and a 128-bit
  shift per word.
- A worker keeps, for each owner, the index of its next letter in that owner's mailbox instead
  of the number of letters it posted there this round. A letter's four stores, and the batch's
  later write of its group, address the mailbox by that index alone, where each record
  computed `(mailbox(half, me, to) * MAILBOX + sent[to]) * 4`. The counts come from the
  indices once per round.

`perf stat -r 10`, twice: instructions:u 1.667 → 1.594 G, 22 fewer per record, and cycles:u
0.91 → 0.87 G. S 294 → 296. `ab.py` against step 24, 40 interleaved runs, medians / minimums:
39.27 / 38.23 → 37.27 / 36.41 ms and 39.31 / 38.45 → 37.46 / 36.52 ms; in two earlier sessions,
on a build that differed only in the type of the window, 39.23 / 38.56 → 37.33 / 36.55 ms and
39.40 / 38.48 → 37.45 / 36.82 ms. memory.peak, 17 alternating rounds: medians 12.63 → 12.77 MB,
means 12.84 → 12.69 MB, the same within this machine's spread. Correctness: the gate, 310 pass,
0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; the 402 fuzz cases pass at the
default thread count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12
and 64, where that build now asserts that no worker's index runs past its mailbox; the 300 tiny
seeds pass at 10 thread counts with 100-byte rounds; of step 11's region sets, the one with
shuffled ids passes, and the two with 689 and 692 countries stop with the error; clippy reports
no warnings.
The board's run, run-b89d1be25a5e, drew 34.8 ms, against 34.9 ms for step 24's: one draw on the
board does not show the 5% measured here.
A second run of the step 25 code, run-835825f2db93 (commit 7128f44, which changed only track A),
measured 34.4 ms, 12.7 MB and 76.9 points.

## Step 26: the long-name compare out of the probe

`NameSet::holds` compared the text of two long names, which only names over 24 bytes need, in
the same expression as the 32-byte key compare. Inlined into `NameSet::read`, the slice bounds
checks of that compare were hoisted out of the probe loop into the setup of every letter, about
a dozen instructions and a few spills per letter for names that are almost never long. The
compare is now `same_text`, a cold function that the probe calls only when two long names'
keys match.

`perf stat -r 10`, twice: instructions:u 1.592 → 1.533 G, 18 fewer per record, and cycles:u
0.860 → 0.837 G. S 296 → 299. `ab.py` against step 25, 40 interleaved runs, medians / minimums:
37.20 / 36.45 → 36.76 / 35.92 ms and 37.52 / 36.37 → 36.69 / 35.94 ms; on a build whose
`same_text` named the two slices through a closure, 37.39 / 36.66 → 36.48 / 35.80 ms and
37.30 / 36.54 → 36.81 / 35.71 ms. memory.peak, 10 alternating rounds: medians 12.37 → 12.59 MB,
means 12.49 → 12.69 MB, inside this machine's spread of 12.1 to 13.6 MB for both. Correctness: the
gate, 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; the 402 fuzz cases,
two of them sets of long names, pass at the default thread count, at 1, 3, 12, 37 and 64
threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300 tiny seeds pass at 10 thread
counts with 100-byte rounds; the region sets give step 25's results; clippy reports no
warnings.

## Step 27: a plain static executable

`-C relocation-model=static` joins `+crt-static` in `.cargo/config.toml`, as in track A: the
binary is a plain static executable at a fixed address instead of a static PIE that relocates
itself before `main`.

Local, turbo off: `ab.py` against step 26, 40 interleaved runs, five rounds: medians 36.54 →
36.47, 36.65 → 36.49, 37.50 → 36.60, 36.47 → 36.17 and 36.61 → 36.53 ms, minimums 35.92 → 35.57,
35.83 → 35.86, 35.53 → 35.73, 35.61 → 35.60 and 35.85 → 35.56 ms. The third round ran on a busy
machine (means 49.9 and 45.5 ms). S unchanged at 299. Gate: 310 pass, 0 fail.

## Step 28: a record's quotes from one mask

`text_field` jumped from quote to quote, and each jump compared the next 32 bytes with `"` and
branched on what it found: a name and a note took at least two such scans, and every `""` one
more. Now the hot loop compares the 64 bytes from a record's name with `"` once, two AVX2
compares packed into one 64-bit mask (`quotes`), and both fields take their quotes from that
mask, lowest first, with `tzcnt`. An escaped quote is two neighbouring bits, cleared together,
and the closing quote is one whose next bit is clear. On full seed 1 the 64 bytes hold the
name's and the note's closing quotes for 97.8% of records (from the name's start to the note's
closing quote: median 37 bytes, 99th percentile 65). A field that runs past the mask moves it
on, to its last bit's quote when that bit is set, because the byte after that quote lies past
the mask and could be its pair. `PAD` grows from 32 to 64 bytes, so the 64-byte read
always fits.

The compares are written with intrinsics. As a loop over the 64 bytes, or as two calls of the
32-byte loop that step 6 used, the compare stayed scalar once inlined into the worker, at about
2.9 G instructions in all, and as a closure it was called out of line twice per record.

`perf stat -r 10`, twice: instructions:u 1.536 → 1.501 G (11 fewer per record), cycles:u
0.839 → 0.767 G, branch misses 3.71 → 2.87 M. S 299 → 311. `ab.py` against step 27, 40
interleaved runs, medians / minimums: 36.45 / 35.38 → 33.79 / 33.13 ms and
36.22 / 35.34 → 33.79 / 33.24 ms; the same source on step 26's static PIE build, against step
26: 36.40 / 35.72 → 34.10 / 33.20, 36.60 / 35.68 → 34.28 / 33.20,
36.99 / 35.75 → 34.41 / 33.43 and 36.39 / 35.77 → 34.16 / 33.44 ms. memory.peak, alternating
rounds: against step 27 (8 rounds) medians 12.82 → 12.89 MB, means 12.98 → 12.77 MB; against
step 26 (10 rounds) medians 13.54 → 12.51 MB, means 13.18 → 12.72 MB; all inside this
machine's spread of 11.7 to 14.0 MB, and no allocation changed. Correctness: the gate, 310 pass,
0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; the 402 fuzz cases pass at the
default thread count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12
and 64; the 300 tiny seeds pass at 10 thread counts with 100-byte rounds; the region sets give
step 27's results; clippy reports no warnings.

## Step 29: plain loads and a cold insert in the mark loop

`NameSet::read` took each letter as four atomic loads, which the compiler may not merge, and
then spent seven more instructions putting the 32-byte key in one vector for the compare in
`holds`. A round's letters are read only after the barrier their posters passed, and none is
stored again until every worker has passed the next one, so `read` now views its mailbox as
plain `[u64; 4]` letters and loads each key at once. `insert`, inlined into the probe, took its
registers and spilled every letter's key; a name is new only once, so `insert` is now `#[cold]`
and out of line. And a letter's group is read as a byte, which it always fits (`parse_regions`
stops at 255 countries), so the word of its bit takes no bounds check.

`perf stat -r 5`, twice: instructions:u 1.505 / 1.511 → 1.450 / 1.440 G, 17 fewer per record;
without the cold `insert`, 1.481 G. S 311 → 313. `ab.py` against step 28, 101 interleaved runs,
medians / minimums: 35.09 / 33.13 → 34.21 / 32.56 ms and 35.18 / 33.29 → 34.57 / 32.78 ms; the
same source without `#[cold]` against with it, 34.76 / 32.85 → 34.70 / 32.43 ms and
34.63 / 32.79 → 33.90 / 32.35 ms; on step 27, against step 27, 36.35 / 35.53 → 35.91 / 35.09 ms
and 36.33 / 35.53 → 35.81 / 35.07 ms. memory.peak, on step 27, 10 seeds × 11 alternating
rounds: the per-seed medians move by +0.07 MB on average (-0.48 to +0.85 MB), inside this
machine's spread, and no allocation changed. Correctness: the gate, 310 pass, 0 fail; full
seeds 1 to 7 and 10 to 12 give the reference hashes; the 400 fuzz cases pass at the default thread
count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300
tiny seeds pass at 1, 3, 12 and 64 threads with 100-byte rounds; 40 generated sets of names
pass at 1 to 4 CPUs; clippy reports no warnings.

## Step 30: 5 fewer lines

Cuts on step 29 in the name set, `merge` and `main`. Three leave the machine code as it was:
`.text` is byte-identical to step 29's binary, and `.rodata` differs only in the line numbers of
`src/main.rs` panic locations (`tools/rodiff.py`).

- `NameSet::insert` takes its key as four words, `[a, b, c, d]`, and builds the entry inside its
  `push`.
- `NameSet::read` takes each letter as its four words and the group and the key from them in one
  statement.
- `main` imports `fs::write`, so the output's write and its error fit one line.

Two change the machine code:

- `main` reads each directory as `std::env::args().nth(i)`, with no iterator kept for the two.
  Only `main`'s code changes, and the code after it moves.
- `merge` sets the min and the max in one assignment, which swaps two stores in the record loop.
  Step 21 dropped this cut because the machine code changed; it measured the same here.

`ab.py` against step 29, 101 interleaved runs per build, three rounds, medians: step 29 33.48,
33.57 and 33.77 ms; the three identical cuts 33.66, 33.50 and 33.86 ms, which is the spread of
the same code; with `merge` 33.58, 33.53 and 33.72 ms; with `nth` 33.50, 33.45 and 33.70 ms; all
five 33.48, 33.55 and 33.59 ms. Two rounds of 40 runs, step 29 against all five: 34.86 → 34.78
and 37.90 → 36.99 ms, on a busy machine. S 313 → 308. Gate: 310 pass, 0 fail; clippy reports no
warnings; a missing input directory, a missing output directory and no arguments stop with the
same errors as before.

Dropped: `format_output` building each line with one `format!` and collecting the lines, 3
lines. With no `write!` into a `Vec<u8>` left, the binary loses the drop glue of `io::Write`'s
adapter, which sits in front of the worker, so the worker's code moves by 16 bytes; on step 28
that build measured 0.26 to 0.52 ms slower than the same build without it, in three rounds. Also
dropped: the rehash's probe as one `find` over `hash..`, 2 lines, which clippy's
`maybe_infinite_iter` flags.

## Step 31: the sources mounted, not copied

The Dockerfile's build stage mounts its context (`RUN --mount=type=bind,target=/src`) instead of
copying it with `COPY . .`, and cargo builds into `--target-dir /out`, since the mount is
read-only; track A does the same since its step 45. `harness/dockerfile_check.py` accepts a bind
mount of the context, and every builder that builds us is BuildKit: Depot, the board's rootless
BuildKit fallback and final scoring's `docker build`. A local `docker build --network=none` of
the old and the new Dockerfile gave binaries byte-identical to each other and to a cargo build of
step 30, so T and M cannot move. S 308 → 307. Gate: 310 pass, 0 fail.

## Step 32: the probe ends at its line

`NameSet::read` probed for an entry's index and then indexed the entries with it once more to
mark the bit. On a hit `holds` had already checked that index, but the check came again after
the loop, where the index that `insert` returns joins it. The probe now ends at the line itself
and `insert` returns the line it pushed, so a hit's index is checked once. The test for a long
name, `key[3] >> 32 == LONG`, moved from `holds` into `same_text`, which only runs when two keys
share their hash and length: in the probe, its two 64-bit constants held two registers for every
letter.

instructions:u on one E-core, so one worker and no waiting: 1428.20 → 1402.29 M, 7.0 fewer per
record; the line alone 1408.85 M, the long-name test alone 1411.93 M. `perf stat -r 5` on the
P-cores, twice: 1.442 / 1.443 → 1.415 / 1.417 G. `ab.py` against step 31, which has step 30's
binary, 101 interleaved runs, medians / minimums: 33.50 / 32.31 → 33.05 / 32.07 ms and 33.59 /
32.60 → 33.17 / 32.50 ms. The worker's code stays where it was; only `NameSet` and `same_text`
change. S 307 → 309. No allocation changed. Correctness: the gate, 310 pass, 0 fail; full seeds
1 to 7 and 10 to 12 give the reference hashes; the 400 fuzz cases pass at the default thread count, at
1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300 tiny seeds
pass at 1, 3, 12 and 64 threads with 100-byte rounds; 40 generated sets of names pass at 1 to 4
CPUs; clippy reports no warnings.

Dropped, by instructions on one E-core against this step's 1402.29 M: the key compare as an OR
of XORs, which keeps the key out of memory but has LLVM build it from four scalars, 1444.50 M;
`holds` borrowing the key, 1418.49 M; `insert` taking the letter where it lies, 1435.00 M; and
`same_text` taking the key's offsets by value, 1402.29 M, the same code.

## Step 33: one pool for the regions and the join

Step 32 built the region table on a pool of its own before the join: each thread listed the rows
of one share of `regions.csv`, then the main thread numbered the codes from the lists and wrote
the array, and only then started the join's threads. On an instrumented copy of step 32 (local,
P-cores, ms after `main` began), the region threads ran from 0.41 to 0.97, the numbering ended
at 1.51 and the array's writes at 2.35, and the join's workers started at 2.45 to 2.63. Now the
join's workers build the table before their rounds, with no lists, in two steps that end at the
barrier the rounds use, which moved into `barrier`. In the first, each worker reads its share of
the rows (`region_rows`) and ORs a bit per country code into a shared set of 26³ bits. In the
second, every worker numbers the codes from the bits, in index order as the serial pass did, and
reads its rows again to write each id's group. With a timestamp at the start of each worker's
rounds, the rounds began 1.33 to 1.70 ms after `main` began, against 2.44 to 3.73 ms for step 32
(4 runs each). The name sets are no longer freed, either: each worker's `munmap` calls stopped
the other workers for a TLB flush, and the exit unmaps the pages anyway.

Mapped in by every worker, the array's pages raised memory.peak by 1.1 MiB, 13.04 against 11.91
MiB (medians, 24 alternating runs), and by 0.6 to 1.4 MiB on each of full seeds 1 to 7 and 10 to 12
(3 runs each), though `smaps` at exit showed less anonymous memory than step 32's. memcg charges
a CPU 64 pages at a time and keeps the rest of the batch for that CPU's next pages, and
memory.peak counts the whole batch. At the end of the rounds memory.current exceeded `anon` +
`kernel` in memory.stat by a median 2.1 MiB, against 1.2 MiB for step 32 with the forget (12
and 18 runs). More pages per worker confirmed it: 16 or 32 more each, 0.75 or
1.5 MiB in all, left the peak at 13.10 and 13.16 MiB (13.10 without them), and 48 raised it to
15.27 MiB. So the main thread maps the array's pages in before the spawn with
`madvise(MADV_POPULATE_WRITE)`, on one CPU as step 32's writes were: 11.79 against 11.74 MiB
(medians, 30 alternating runs; the highest of 5 runs, which the board takes, 12.20 against 12.15
MiB by resampling). The table of code numbers, 17.5 KB per worker, is freed before the worker's
groups are made, which reuse its pages: 0.23 MiB less `anon`.

`ab.py` against step 32, 50 interleaved runs, two rounds, medians / minimums: 32.60 / 31.98 →
31.58 / 30.87 ms and 33.04 / 32.17 → 31.82 / 31.01 ms. instructions:u on one E-core: 1402.29 →
1403.90 M, the rows now read twice. S 309 → 319. Correctness: the gate, 310 pass, 0 fail; full
seeds 1 to 7 and 10 to 12 give the reference hashes; the 400 fuzz cases pass at the default thread
count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300
tiny seeds pass at 1, 2, 3, 5, 7, 12, 16, 23, 37 and 64 threads with 100-byte rounds; clippy
reports no warnings.

Dropped: the pages written from the main thread one byte each, the same memory.peak and 0.16 to
0.28 ms slower (`ab.py`, 50 runs, three rounds); the pages written by worker 0 during the first
step, 12.43 against 11.96 MiB for step 32 (16 runs); and the array written by worker 0 alone in
the second step, 12.89 against 12.30 MiB for step 32 with the forget (6 runs). Freeing the name
sets as before measured 32.04 / 31.57 and 32.09 / 31.60 ms against 31.94 / 31.38 and 32.03 /
31.57 ms with the forget (50 runs, two rounds), with memory.peak 12.11 against 11.86 MiB (24
runs).

## Step 34: 44 fewer instructions per record

Step 33 ran 427 instructions per record, and a profile of `uops_issued.any` by source line (a
`profiling` build of step 33, 20 runs) found no block that dominates: the three number parses
were 18% of the samples, the name set's key compare 11%, and the posting, the batch and the
bounds checks a few percent each. The cuts, each measured as instructions:u and cycles:u with
`perf stat` (20 interleaved runs against step 33, medians):

- **The amount as one number.** `number` takes a count of decimals after its stop. The word is
  shifted so that the stop lands in byte 7 minus the decimals for every length, and the bytes
  below it move up over it, so the same three multiplies give the cents. §2.2 bounds the amount
  at 99,999.99, so its digits and its dot fit in one word; a longer one takes the byte loop,
  which reads the decimals too. -0.4% instructions, -1.3% cycles.
- **The batch made once per worker** instead of zeroed for every batch: with the amount,
  -0.8% / -2.5%.
- **`sent[to]` read once.** The four stores of a letter made the compiler load it again for
  each, with a bounds check. On step 32's code this measured slower (see the experiments below);
  on top of the amount it measured -4.7% / -3.3%, and borrowing the letter's slot once instead,
  `&boxes[sent[to]]`, -4.3% / -2.9%.
- **The BOM test from the id's word.** A record starts with a digit or a BOM, so the id is parsed
  first and parsed again from past the BOM when its first byte is 0xEF, which the id's word
  already holds: no load and no bounds check for the test. On top of the amount, -2.7% / -2.7%.
  The four together: -7.4% / -5.1 to -5.8% (three sessions).
- **The stop as the first byte below `0`.** Subtracting `0` from every byte replaces the XOR with
  the stop: every stop, `,` or `.`, lies below `0`, and the difference already holds each digit's
  value, so an id and a region id need no mask. On top of the four: -10.8% / -7.4 to -8.0%.
- **The input prefetched 448 bytes ahead** of each record. PEBS `mem_load_retired.l1_miss` on a
  `profiling` build put 10.1 M L1 misses on a run: 73% on the name set (its slots 27%, the key
  compare 26%, the group's distinct count 20%), 18% on the groups' merge and 7% on the input
  (the quote masks' 64-byte loads and the number words). With the prefetch, L1 misses 10.11 →
  9.39 M, instructions +0.5% and cycles -1.1% (40 interleaved runs); 384 bytes ahead, -1.0%.

On AC power again (profile `performance`, turbo off), `ab.py`, 41 interleaved runs, two
rounds: medians 32.26 and 32.15 → 29.84 and 29.81 ms, minimums 31.34 and 31.29 → 29.13 and
28.96 ms, 7.1 to 7.5% faster. The counters were measured on battery power before that (local,
P-cores): `perf stat`, 40 interleaved runs, two rounds, instructions:u 1418.0 → 1271.2 M (-10.4%),
cycles:u medians 768.5 → 703.0 M and 764.2 → 699.1 M (-8.5%), minimums 758.3 → 694.4 M and
754.6 → 694.1 M. memory.peak, 30 alternating runs: medians 12.01 → 11.96 MiB, the highest of 5
by resampling 12.52 → 12.37 MiB. S 319 → 326. Correctness: the gate, 310 pass, 0 fail; full seeds
1 to 7 and 10 to 12 give the reference hashes; the 400 fuzz cases and the two long-name sets pass at
the default thread count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12
and 64; the 300 tiny seeds pass at 1, 2, 3, 5, 7, 12, 16, 23, 37 and 64 threads with 100-byte
rounds; clippy reports no warnings. The board's run, run-91b80b654f7a, measured 26.9 ms and
12.2 MB, against 29.1 ms and 12.2 MB for step 33's.

Branch misses, 0.87 per record (2.87 M per run), are 55% the name set's probe, which step 32's
eight-slot probe already took on with no gain (see the experiments below), and 34% the pair test
in `text_field`, on names and notes with an escaped quote. A branchless pair test costs more
instructions than the misses' cycles, which the hyperthread sibling partly hides.

Dropped, on battery power, `perf stat`, 20 interleaved runs against the code before them:
- Each letter posted right after its name, so that its four words were not held across the note
  and the numbers: instructions -2.7%, but cycles +2.0%, and backend-bound slots 22.0 → 26.6%.
- The batch holding a reference to the letter's last word instead of its slot: -0.2%
  instructions, +1.0% cycles.
- The owner count in a local, instead of the captured thread count: +0.3% instructions, cycles
  within noise.
- A zero byte past the region table, so that the lookup takes `min` instead of `get`: +1.3%
  instructions, +1.8% cycles.
- Distinct names counted in a dense array per worker, 2 KiB, instead of in the groups, whose
  lines took 20% of the L1 misses in the marking loop: -1.1% instructions, cycles within noise.

## Step 35: 65 fewer instructions per record

Step 34 ran 383 instructions per record, and its profiles by source line pointed at the field
parses: the two number parses of the region id and the amount, each a chain of shifts,
multiplies and branches, and the walk over each field's quotes. The cuts, each measured with
`perf stat` against the code before it (20 to 30 interleaved runs, medians; on battery power at
first, so instructions:u and cycles:u only), and then timed again on AC power with `ab.py` (41
interleaved runs, two rounds, medians, P-cores):

- **The region id and the amount from one vector.** §2.2 bounds region ids at 9,999,999 and
  amounts at -999.99 to 99,999.99, so the two fields fit in 16 bytes. One SSE compare finds the
  comma and the dot, one `pshufb`, with a control picked by their places, right-aligns both
  numbers' digits with the dot dropped, and three multiply-adds give both values at once. Fields
  past those bounds that §2.1 still allows take two number parses, as before. `number` loses its
  decimals, which only the amount used. Instructions -9.0%, cycles -5.2 to -5.7% (two sessions);
  `ab.py` -5.2% and -5.3%.
- **A CRC32 name hash** instead of `FxHash`: one `crc32` per word and no fold, and its 32 bits
  depend on every word. -3.5% / -1.7%, with branch misses the same (2.91 → 2.92 M), so the names
  spread over the slots as well; `ab.py` -1.2% / -1.4%.
- **The name set's key in one AVX2 register**, from one load and one AND of the letter, compared
  with an XOR and `vptest` against the first 32 bytes of the entry. The cold insert and the
  long-name compare take the key as a vector too: when they took four words, the probe stored
  every letter's key to the stack for them (+1.2% instructions, +1.0% cycles). -2.7% / -1.4%;
  `ab.py` -1.0% / -1.7%.
- **`sent[to]` without its bounds check.** `to` is a 32-bit hash times the thread count, shifted
  down by 32, so it is below the thread count. -1.7% / +0.1%, `ab.py` -0.2% / +0.1%; but on the
  final code, putting the check back cost 2.2% more instructions and 1.4% more cycles, so it stays.
- **The name and the note from one mask of the 64 bytes at the name.** The prefix XOR of the
  quote mask, one carry-less multiply by all ones as in simdjson, marks the bytes inside quotes;
  the first two commas outside end the two fields, and a field's length is its raw length less
  half its quotes. There is no branch per quote, where `text_field` walked the quotes one by one
  and mispredicted on escaped ones. The 2.2% of records whose note ends past the window take a
  loop over 64-byte blocks that carries the parity from block to block. -2.0% / -2.6%, branch
  misses 2.89 → 1.89 M, branches 136.8 → 107.1 M; `ab.py` -2.4% / -2.3%.
- **The `pshufb` controls 16-byte aligned**, as vectors, so that none straddles two cache lines:
  cycles -0.2%, `ab.py` within noise.

On AC power (profile `performance`, turbo off), `ab.py`, 41 interleaved runs, two rounds, against
step 34: medians 29.66 and 29.46 → 26.72 and 26.50 ms, minimums 28.66 and 28.72 → 25.73 and 25.69
ms, 9.9 to 10.6% faster. `perf stat`, 20 interleaved runs: instructions:u 1272.1 → 1055.2 M (-17.0%,
383 → 318 per record), cycles:u 684.4 → 601.9 M (-12.0%). memory.peak, 30 alternating runs, two
rounds: medians 12.00 → 12.19 MiB and 12.05 → 12.17 MiB, the highest of 5 by resampling 12.29 →
12.54 MiB and 12.41 → 12.67 MiB. S 326 → 387: the step adds 61 lines, kept down by one glob import
for the intrinsics and by the loss of `text_field` and `quotes`. Correctness: the gate, 310 pass, 0
fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; the 400 fuzz cases and the two long-name
sets pass at the default thread count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at
1, 2, 3, 12 and 64; the 300 tiny seeds pass at 1, 2, 3, 5, 7, 12, 16, 23, 37 and 64 threads with
100-byte rounds; six generated files of edge cases (region ids of 1 to 8 digits against amounts of 1
to 7, with signs and leading zeros, and names and notes whose quotes, escapes and commas fall around
the 64-byte window and the 24-byte letter) match the reference at 1, 2, 3, 7, 12 and 64 threads and
with 100-byte rounds; clippy reports no warnings.

A profile of cycles by part on the step's code (a `profiling` build, 60 runs): marking the
letters (`NameSet::read`) 27.4%, the rest of the record loop 9.6%, `region_amount` 7.8%, the
barrier 7.6%, `text_fields` 7.3%, the merge 4.4% and `name_letter` 3.9%.

Dropped, `perf stat`, 20 interleaved runs against the code before them, on battery power unless
noted:
- The id parsed by SSE too: instructions +0.1%, cycles the same.
- The input prefetched 576 or 704 bytes ahead instead of 448: instructions +0.02% / +0.09%,
  cycles medians +1.6% / +2.6%, minimums the same.
- The marking loop prefetching the slots and entries of the letters 8 or 16 ahead, two shapes:
  L1 misses 9.7 → 5.5 M and L2 misses 2.65 → 0.83 M per run, but instructions +8.6 to +9.0% and
  cycles +0.6 to +1.7%, and the marking took as long, so it does not wait on those misses.
- The probe's slots read without bounds checks: instructions -0.3%, cycles +0.3%.
- The slow text path counting its quotes byte by byte, which takes fewer lines (on AC power, on
  the final code): instructions +1.3%, cycles +1.5%; 2% of the records is enough for that.

The board's run, run-6252826712db, drew 25.2 ms against 27.2 ms for step 34's second run,
run-835b63d0f252, and 26.9 ms for its first: -6 to -7%, less than the -10% measured here, while
the 61 more lines cost about as many points as 5% of T.

## Step 36: the batch's loads forwarded from its stores

The record loop stores each record's fields into the batch of four and loads them back for the
merge once the batch is parsed. In step 35's code the compiler had put two of them, the id and
the amount, side by side, and loaded them as one 16-byte vector. A load that spans two stores
still in the store buffer cannot take its bytes from them, so it waits until both reach the
cache: `ld_blocks.store_forward` counted 0.44 to 0.64 M such loads per run, almost all in the
merge. Three changes, none of which adds a line:

- **The batch's fields in another order.** A tuple's layout is the compiler's choice. In this
  order the id, the amount and the name's length, the three values that the merge puts into one
  vector with the count, lie apart, so each is loaded on its own: store-forward blocks 0.44 M →
  3.3 k per run. Alone it built the vector from three loads instead of two: instructions +1.4%,
  `ab.py` +0.7%.
- **The group as one byte.** The group went into the letter's last word with a load, an OR and a
  store. One byte store now writes byte 30, bits 48 to 55, which `name_letter` leaves 0. With the
  new order, `ab.py` -0.2% to -1.1% against step 35 in six rounds.
- **The letter by reference.** The batch holds a reference to the record's letter instead of its
  slot, so the group's store takes no second bounds check or address computation.

On AC power, `ab.py`, 41 interleaved runs, two rounds, against step 35: medians 26.99 and 27.01
→ 26.39 and 26.54 ms (-2.2% and -1.7%), minimums 26.24 and 26.13 → 25.89 and 25.92 ms; three
earlier sessions of the same code measured -2.4%, -1.6% and -2.2%. `perf stat`, 20 interleaved
runs: instructions:u 1055.2 → 1024.6 M (-2.9%, 318 → 309 per record), cycles:u 599.9 → 590.1 M
(-1.6%). memory.peak, 15 alternating runs: medians 12.55 → 12.36 MiB. S 387 → 387. Correctness:
the gate, 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; the 400 fuzz
cases and the two long-name sets pass at the default thread count, at 1, 3, 12, 37 and 64
threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300 tiny seeds pass at 1, 2, 3, 5,
7, 12, 16, 23, 37 and 64 threads with 100-byte rounds; the six files of edge cases match the
reference at the default thread count, at 1 and 37 threads, and with 100-byte rounds at 3 and 64;
clippy reports no warnings.

Where step 35's slots go, top-down at level 2 (`perf stat --topdown`, P-cores): retiring 54%
(light operations 51.2%, heavy 2.7%), bad speculation 8.6% (branch mispredicts 8.3%), frontend
11.6% (latency 8.4%, bandwidth 3.2%) and backend 24.8% (core 17.0%, memory 7.8%). Of the 1.83 M
branch misses per run, 94% are in `NameSet::read` and 86% at one branch, the tag compare of the
probe: with the slots half full, a letter passes half a slot of other names on average before
its own, and each such pass is a likely miss.

Dropped, against the code before them; `ab.py` on AC power, 41 interleaved runs, two rounds, and
`perf stat`, 20 interleaved runs:
- **The letter as one AVX2 vector**, from one 32-byte load and one AND, and posted with one
  32-byte store, with its three words taken out of the vector for the hash: instructions -5.7%,
  but cycles +1.4% (on battery power) and `ab.py` +1.3% and +1.4%; with the group stored as one
  byte, +0.9% and +0.5%. Top-down: frontend latency 8.4 → 9.6 to 11.1% of the slots. Step 32's
  letter posted right after its name went the same way.
- **One text path for every record**, the loop over 64-byte blocks that 2% of the records take,
  which saves 18 lines (387 → 369): instructions +13.9%, cycles +5.0%.
- **Non-temporal prefetches** (on battery power, `perf stat` only): of the input, cycles +4.7%; of
  the region table, +2.4%; of both, +7.2%.
- **Batches of 6 and 8 records** instead of 4, on this step's code, three rounds: 6 0.0% to
  +0.1%, 8 -0.1% to -0.7%. Batches of 3, before the byte store and the reference: +1.6% and
  +1.8%.
- **The distinct names counted from the bits after the last round** (not built): the marking's
  test and count are already one `bts` and one `sbb` on the group's count, and a pass over the
  bits would visit every set bit, 0.9 per letter, since 89.8% of the records add a pair.
- **The merge as vectors** (not built): the compiler already adds the record's first four fields
  to its group with one AVX2 add.

## Step 37: a bounds check and a load fewer per record

Two changes in the record loop. Neither adds a line. Both came from reading the loop's machine
code next to a `perf record` of its user-mode instructions over 40 runs, with each sample charged
to its line:

- **The group's index unchecked.** The merge indexed `groups` with the group read from `by_id`,
  and the compiler compared it with the length of `groups` first, two instructions per record.
  The check can never fail, whatever the input: step 1 writes into `by_id` only the number of a
  code that occurs, which is below `group_count`, the length of `groups`, every other byte stays
  0, and an id past the table maps to 0. Alone: instructions -0.66%, cycles -0.80%.
- **The thread count as `sent.len()`.** A letter goes to the thread given by its hash times the
  thread count, shifted down by 32. The worker's closure holds `threads` by reference, so the
  multiply waited on two loads, one after the other: the reference from the stack, then the count
  through it. `sent` holds one mailbox position per thread, so its length is the same count, and
  the multiply now reads it from the stack in one load. On top of the first change: instructions
  -0.65%, cycles -0.47%.

On AC power, `ab.py`, 41 interleaved runs, four rounds, against step 36: medians 26.35, 26.50,
26.41 and 26.29 → 26.19, 26.30, 26.41 and 26.19 ms (-0.6%, -0.8%, 0.0% and -0.4%). Six earlier
rounds of the same machine code, built before the comments were written, measured -0.2% to
-1.0%. `perf stat`, 20 interleaved runs: instructions:u 1024.4 → 1011.1 M (-1.3%, 309 → 305 per
record), cycles:u 586.2 → 581.3 M (-0.8%). memory.peak, 15 alternating runs: medians 12.36 →
12.39 MiB. S 387 → 387. Correctness: the gate, 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give
the reference hashes; the 400 fuzz cases and the two long-name sets pass at the default thread
count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300
tiny seeds pass at 1, 2, 3, 5, 7, 12, 16, 23, 37 and 64 threads with 100-byte rounds; the six
files of edge cases match the reference at the default thread count, at 1 and 37 threads, and
with 100-byte rounds at 3 and 64; clippy reports no warnings.

The loop's other bounds checks stay. Each guards a read at a stop found by the parse before it,
and a malformed last record can put such a stop at the end of the input, where the read would
run past the padding.

Dropped, against step 36 unless noted; `ab.py` on AC power, 41 interleaved runs, and `perf stat`,
20 interleaved runs:
- **The name set's tables as local slices** in `NameSet::read`, taken again after each insert, so
  that the probe would not load the slots' address and length through `self` for every letter:
  instructions +1.1%, cycles +2.6%, `ab.py` +2.1% and +2.6%.
- **The group unchecked in the marking too**, the distinct count after the barrier, which takes
  one more line: instructions -1.0%, but cycles -0.6% and -0.8%, no better than the merge's
  change alone, and in three rounds of `ab.py` with both, +0.3%, -0.2% and +1.0% against it.
- **The id's word read unchecked** (`perf stat` only): with every read of `number` unchecked,
  which is not sound, instructions -1.85% but cycles -0.45%, against the merge's change alone.
  Only the reads at a record's start can be shown in bounds for any input. The slow amount parse
  also calls `number`, from stops that a malformed last record can put at the end of the input,
  and keeping its check takes more lines.
- **The letter made after the region id and the amount**, on top of both changes: instructions
  -0.31%, cycles +0.42%.

## Step 38: the last worker writes the output, with no runtime setup

Step 37's workers returned their groups, and the main thread merged them, wrote the output and
exited once every worker's thread had ended. On an instrumented copy of step 35 (local, P-cores,
µs after the fork, medians of 21 interleaved runs), the workers passed the last barrier at 23,520
and had all given back their runs by 24,007, but the last thread ended only at 24,773: a thread
that ends unmaps its signal stack and has glibc drop its stack's pages, each call takes the
address space's lock, and so the twelve ended one after another. The merge, the format and the
write took 405 µs more, and the exit 646.

Now no worker returns. Each puts its groups in `parts`, and the last one to get there merges the
others into its own, writes the output and calls `exit`, while the others sleep in
`thread::park` until the exit ends them. Threads killed asleep cost the exit about 0.3 ms that
threads already ended did not (629 against 343 µs from the `exit` call to the wait's return), less
than half of what ending them took. Under gdb on the E-cores (4 threads), a run makes 4 `munmap`
and 10 `sigaltstack` calls instead of 10 and 14. Since `join` never returns, its mailboxes and
region table are never freed either.

Each worker also gives back its name set's entries, 6.4 MB in all at full scale, with
`madvise(MADV_DONTNEED)` on the whole pages inside them, after its run. The exit used to unmap
them on one thread: from the `exit` call to the wait's return took 1,064 µs without this and 629
µs with it, while the twelve calls, in parallel, took about 127 µs. From the last barrier to the
wait's return, the tail took 2,289 µs on step 35, 1,689 µs with the last worker writing, and
1,446 µs with the name sets given back too (with their slots, see below; 21 runs each). On step
36 it took 2,496 µs before and 1,541 µs after (25 runs). The name set itself is still never
freed (step 33).

`ab.py` against step 37, 41 interleaved runs, four rounds: medians 26.09, 26.26, 26.37 and 26.10
→ 25.66, 25.64, 25.69 and 25.57 ms (-1.6%, -2.4%, -2.6% and -2.0%). memory.peak does not move,
as every page goes back after the peak: in a user scope on the P-cores, 80 alternating runs,
medians 12.31 → 12.33 MiB, and the highest of 5 runs by resampling 12.79 → 12.68 MiB; in the
harness's container on the E-cores (4 threads), 50 alternating runs, 10.52 → 10.48 MiB. S 387 →
391. Correctness: the gate, 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes;
the 400 fuzz cases and the two long-name sets pass at the default thread count, at 1, 3, 12, 37
and 64 threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300 tiny seeds pass at 1, 2,
3, 5, 7, 12, 16, 23, 37 and 64 threads with 100-byte rounds; the six files of edge cases match the
reference at the default thread count, at 1 and 37 threads, and with 100-byte rounds at 3 and 64;
of the wide region sets, w2 gives the reference, and w0 and w1, with more than 255 countries,
stop with the error as step 37 does; clippy reports no warnings.

`main` is now C's `main`, named with `#[unsafe(no_mangle)]` under `#![no_main]`, so the setup
that the standard library runs before a Rust `main` does not run. That setup installs a handler
for stack overflows, and while it is installed, every spawned thread maps and protects the pages
of a signal stack of its own as it starts, two calls that take the address space's lock for
writing while the main thread is still spawning. Under gdb on the E-cores (4 threads), a run
makes 22 `mmap` and 9 `mprotect` calls instead of 27 and 14, and `sigaltstack` is not linked at
all. `std::env::args` still works, because glibc hands the arguments to the standard library's
initializer before `main`. A stack overflow now ends the process with SIGSEGV and no message, and
SIGPIPE keeps its default action, which a program that writes one file never meets. The binary
is 122 KiB smaller. `ab.py` against the change above alone, 41 interleaved runs, four rounds:
25.66, 25.64, 25.69 and 25.57 → 25.36, 25.46, 25.44 and 25.37 ms (-1.2%, -0.7%, -1.0% and
-0.8%), and against step 37 -2.8%, -3.0%, -3.5% and -2.8%. memory.peak: 12.33 → 12.23 MiB in the
user scope (80 runs; highest of 5, 12.68 → 12.73), 10.48 → 10.52 MiB in the container (50 runs).
S 391 → 393. The same checks pass, with the same results.

Dropped, against step 35 unless noted; instrumented timelines on the P-cores, medians of 21
interleaved runs (25 on step 36), or `ab.py`, 41 interleaved runs, four rounds:
- **The name set freed** after the run instead of its entries given back (on step 36): `ab.py`
  26.51, 26.52, 26.53 and 26.49 ms against 25.80, 25.88, 25.81 and 25.88 ms, +0.67 ms. `munmap`
  takes the address space's lock for writing, so the workers' calls ran one at a time.
- **The slots given back too**, the set's other table, 64 KiB per worker (on step 36): the tail
  1,489 against 1,541 µs, 52 µs for three more lines.
- **The region table and the mailboxes given back**, each worker its share after one more
  barrier, with the threads still ending as before: the tail 1,817 against 2,289 µs, no better
  than the name sets alone, 1,810 µs. With the last worker writing, the region table alone (on
  step 36): 1,474 against 1,489 µs.
- **The sleeping workers spinning** instead (on step 36): the tail 1,491 against 1,489 µs.
- **The main thread as worker 0**, after it spawned the others (on step 36): the tail 1,509
  against 1,489 µs.
- **The sleeping workers ending their threads with a raw `exit(2)`**: the standard library of
  Rust 1.99 then panics, "threads should not terminate unexpectedly".
- **The join's mailboxes and region table leaked** when it returned, instead of freed: the tail
  2,056 against 2,408 µs. The last worker's exit now gives that with no line.
- **The region table mapped in by worker 0** before its step 0, instead of by the main thread
  before the spawn: the last worker's rounds started at 2,143 against 2,043 µs.
- **`MADV_SEQUENTIAL` or `MADV_RANDOM` on the input mapping**: the last worker's rounds started at
  2,116 and 2,113 against 2,043 µs, and the walls stayed within the noise.
- **The region table mapped in by the main thread after the spawn** instead of before it, so
  that its 0.2 ms would overlap the threads' start (on step 36 with no runtime setup): `ab.py`
  25.44, 25.55, 25.54 and 25.60 → 25.49, 25.66, 25.69 and 25.67 ms, for three more lines.
- **One malloc arena for all threads** (`mallopt(M_ARENA_MAX, 1)`, on step 36 with no runtime
  setup): 30.00, 30.27, 30.23 and 30.21 ms, +4.7 ms.
- **glibc's `M_MMAP_THRESHOLD` left at its default**, two lines fewer (on this step): memory.peak
  12.20 → 12.38 MiB (medians, 80 alternating runs in a user scope), and the highest of 5 runs
  12.63 → 13.14 MiB by resampling.

## Step 39: 23 instructions fewer per record

Eight changes in the record loop and the name set, none of which adds a line. They came, as step
37's did, from reading the loop's machine code next to a `perf record` of its user-mode
instructions over 40 runs, each sample charged to its line. Each count below is `perf stat`'s
instructions:u, 20 interleaved runs, on step 37's code with none or some of the other changes,
against the same code without that change.

Step 37 kept the loop's other bounds checks, since a malformed last record can put a stop of the
parse at the end of the input. Three of them, though, read near a position that an earlier
check or the parse has already bounded, well inside the pad's 64 bytes, so they can never fail.
Each function now has that as the `# Safety` contract of its read, and each call says why it
holds:

- **The name's 24 bytes.** `text_fields` has just read the 64 bytes from the name's field on, its
  first window, with a checked read, and the name starts there or one byte later, past its
  quote, so `name_letter`'s 24 bytes lie in that window. 4 instructions per record (-1.3%).
- **The region id's and the amount's 16 bytes.** `region_amount` reads from one past the comma
  that ends the note. A mask of commas finds only commas, and the 64 bytes of the pad are zeros,
  so that comma is a byte of the input, and the 16 bytes past it end inside the pad at the
  latest. 5 instructions per record (-1.6%).
- **The byte after the cents**, the CR or the LF. `region_amount` puts the dot at most 13 bytes
  past the comma's next byte, and `region_amount_slow` at most at the pad's second byte, so the
  byte 3 past the dot lies in the pad at the latest. 3 instructions per record (-1.0%).

The check that stays is the window's: a last record with no comma after its id puts the id's
stop on the pad's first byte, and the window from the byte after it would end one byte past the
pad.

Five smaller ones:

- **A CR told from an LF by its lowest bit.** CR is 0x0D and LF 0x0A, and the byte after the
  cents is one of them or, after the last record, the pad's 0 (§2.1). One instruction per record
  (-0.33%).
- **The region id's length checked with one compare.** The fast path took region ids of 1 to 7
  digits as `comma.wrapping_sub(1) < 7`. An empty region id, which §2.2 rules out, now takes it
  too and parses as 0, as it does in the slow path, so `comma < 8` is enough. One instruction per
  record (-0.34%).
- **The quotes counted with `bzhi`.** The quotes before a comma were counted as `quotes & ((1 <<
  i) - 1)`. The compiler does not know that `i` is below 64, so it masked the count first;
  `_bzhi_u64` takes any count. 2 instructions per record (-0.63%).
- **A slot's entry above its tag.** A slot held its entry's number in its low 24 bits and 8 bits
  of the name's hash on top, so the probe masked it once for each. With the number on top, one
  shift takes it out and the tag compares as the low byte. 2.3 instructions per record (-0.77%).
- **The marking's group unchecked.** After the barrier, the distinct count indexed `groups` with
  the group that the letter's poster took from `by_id`, which holds 0 or a number below
  `group_count`, the length of every worker's `groups`, since all count the same codes. One
  instruction per record (-0.33%). Step 37 dropped this change, which then took one more line;
  with the word and the bit taken in one `let`, it takes none.

Together, on AC power, `ab.py`, 41 interleaved runs, five rounds, against step 38: medians 25.64,
25.65, 25.60, 25.50 and 25.58 → 25.05, 25.07, 24.97, 24.93 and 25.06 ms (-2.3%, -2.3%, -2.5%,
-2.2% and -2.0%). `perf stat`, 20 interleaved runs: instructions:u 1014.4 → 936.8 M (-7.6%, 306
→ 282 per record), cycles:u 583.8 → 569.8 M (-2.4%). On step 37's code the same changes took
1011.2 → 947.0 M instructions (-6.3%) and 582.6 → 567.7 M cycles (-2.6%), and in seven rounds of
`ab.py` against step 37, -1.7% to -2.7%. Cycles fall by less than instructions: many of the
instructions cut were a compare and a branch that never jumps, which the core fuses into one
operation. memory.peak, 40 alternating runs: medians 12.36 → 12.33 MiB. S 393 → 393.
Correctness: the gate, 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; the
400 fuzz cases and the two long-name sets pass at the default thread count, at 1, 3, 12, 37 and
64 threads, and with 100-byte rounds at 1, 2, 3, 12 and 64; the 300 tiny seeds pass at 1, 2, 3,
5, 7, 12, 16, 23, 37 and 64 threads with 100-byte rounds; the six files of edge cases match the
reference at the default thread count, at 1 and 37 threads, and with 100-byte rounds at 3 and 64;
of the wide region sets, w2 gives the reference, and w0 and w1, with more than 255 countries,
stop with the error as step 38 does; clippy reports no warnings.

Dropped, on step 37's code; `ab.py` on AC power, 41 interleaved runs, and `perf stat`, 20
interleaved runs:
- **The probe's slot read unchecked**: one instruction per record fewer (-0.32%), but cycles
  +0.44%, and `ab.py` +0.1% and +0.6% against step 37; on top of the CR's bit, the `bzhi`, the
  slot's layout and the marking's group, +0.3%, +0.2% and +0.5% against them.
- **The letter posted before the region id and the amount are parsed**, on top of those four:
  instructions -0.34%, cycles +2.3%. **The letter made after them**, which step 37 dropped too,
  again on top of those four: instructions +0.35%, cycles +0.63%.
- **The shuffle controls indexed by `dot << 3 | comma`**, which one `lea` could compute, instead
  of `comma << 4 | dot`: the compiler shifted and ORed all the same, instructions -0.00%, cycles
  +0.24%. As a table of 17 dots by 8 commas indexed with no mask, it added a bounds check.
- **The dot found by a shift past the byte after the comma**, so that its range check would be
  one compare: instructions -0.02%.

## Step 40: the field's word read unchecked

`number` parses a field from the 8 bytes at its start, read as one word, and checked on every
call that they lie in `data`. They always do: every caller starts the word at most two bytes
past the input's end, and the pad is 64 bytes. `number` is now an `unsafe fn` whose contract is
that the 8 bytes lie in `data`, and each of its six calls says why they do. The change adds no
line.

- **The id at a record's start**, and the id 3 bytes on after a BOM. The record starts below its
  run's end, which is at most the input's length. The BOM test reads the record's first byte
  unchecked for the same reason: with the word's check gone, the compiler checked that byte
  instead, two instructions per record.
- **The two fields of `region_amount_slow`.** The region id starts one past the note's comma, at
  most at the input's length, and the amount one or two bytes past the region id's stop, which
  is at most the pad's first byte or a comma of the input. `region_amount` and
  `region_amount_slow` now both require that their start be at most the input's length, which
  the record loop knows from the comma before it.
- **A row of the region table**, which starts below the body's end, and **the last row's id**,
  one past an LF that lies before the pad.

On AC power, `ab.py`, 41 interleaved runs, seven rounds, against step 39: medians 25.01, 24.95,
25.08, 25.11, 25.09, 25.10 and 25.07 → 24.80, 24.74, 24.73, 24.84, 24.78, 24.83 and 24.77 ms
(-0.8%, -0.8%, -1.4%, -1.1%, -1.2%, -1.1% and -1.2%). `perf stat`, 20 interleaved runs:
instructions:u 936.6 → 911.0 M (-2.7%, 282 → 274 per record), cycles:u 566.4 → 560.7 M (-1.0%).
memory.peak, 40 alternating runs: medians 12.27 → 12.23 MiB. S 393 → 393. Correctness: the gate,
310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give the reference hashes; the 400 fuzz cases and the
two long-name sets pass at the default thread count, at 1, 3, 12, 37 and 64 threads, and with
100-byte rounds at 1, 2, 3, 12 and 64; the 300 tiny seeds pass at 1, 2, 3, 5, 7, 12, 16, 23, 37
and 64 threads with 100-byte rounds; the six files of edge cases match the reference at the
default thread count, at 1 and 37 threads, and with 100-byte rounds at 3 and 64; of the wide
region sets, w2 gives the reference, and w0 and w1 stop with the error as before; 73 inputs cut
short at every awkward place (inside the id, after one or two bytes of a BOM, inside a quoted
note, inside the region id and the amount, with and without a CR, and region tables cut short
in their last row) give the same exit status, panic message and output as step 39; clippy
reports no warnings.

Where step 39's 282 instructions per record go, from `perf record` of instructions:u with
PEBS's precise distribution (`:ppp`), 120 runs, one sample per 4,000,037 instructions, each
charged to its line of the record loop. With `:pp` instead, the samples piled up behind slow
instructions: the region table's prefetch, one instruction per record, drew 10 per record.

| Part of the record loop | Instructions per record |
|---|---|
| `text_fields`, with `text_fields_slow` (3.4) | 60.8 |
| `region_amount` | 38.0 |
| `NameSet::read` and `NameSet::insert`, the marking | 33.0 |
| `number` for the id, and the BOM test | 31.7 |
| `name_letter`, without its three `crc32`s | 22.0 |
| `merge` of the record into its group | 18.6 |
| the batch: its stores, its loop, the group's lookup and its byte in the letter | 17.8 |
| the letter's four stores into its mailbox | 14.8 |
| the loop's control, both prefetches and the line end | 11.8 |
| the post: the owner, the slot and its count | 8.8 |
| no line in the line tables after inlining: the `crc32`s, reloads of spilled values | 23.8 |
| the rest: rounds, barrier, setup, tail | 1.0 |

Dropped, against step 39; `ab.py` on AC power, 41 interleaved runs, and `perf stat`, 20
interleaved runs:
- **The empty name kept out of the exchange.** At full scale 1.0% of the records have an empty
  name, whose letter is all zeros, so its hash and its owner are 0, and worker 0 marked about
  11% more letters than the others. With no worker posting it, and a byte per group, shared and
  set once, to count it, +4 lines: six rounds from -0.5% to +0.2%, instructions +1.0%, cycles
  +0.0% and +0.3%. With the branch that sets the byte made cold: three rounds from -0.3% to
  +0.2%, instructions +0.9%, cycles +0.0%. Not posting it at all, which leaves it out of the
  counts, as the bound on what balancing worker 0 can gain: three rounds from -0.6% to +0.1%,
  instructions +0.2%, cycles +0.2%.
- **A pad of 128 bytes**, so that the 64-byte window of `text_fields` would be read unchecked
  too: a last record cut short after a BOM puts the id's stop on the pad's third byte, and the
  window from the byte after it ends 67 bytes into the pad. Alone: instructions -1.46% (4 per
  record), cycles -0.17%, three rounds 0.0%, 0.0% and -0.3%. With this step's change, against
  it: instructions -0.77%, cycles -0.02%, seven rounds from -0.2% to +0.2%.

The board's run, run-d1b2c0ca269e (commit ca86ea9), drew 24.0 ms, a slow draw: step 39's code drew
22.8 ms in its own run, and this step runs 0.8 to 1.4% faster locally. A second run of the same code,
run-604f816aac75 (commit 0327cbc, which changed only track A), drew 22.76 ms, 12.5 MB and 80.5 points.

## Step 41: a name known by its hash

Until step 40, a letter carried the name's first 24 bytes, a CRC32 and the length, 32 bytes,
and a longer name as its offsets in the file, and an owner's entry for a name was a 64-byte
cache line: that 32-byte key and 32 bytes of group bits. At full scale the ~100k entries took
6.4 MB, about half of memory.peak.

- **The letter is a hash.** `name_letter` reads the name's 32 bytes as two 16-byte blocks, `a`
  and `b`, masked to its length (names hold no NUL byte, §2.2, so the blocks are one to one with
  the name), and hashes them in three instructions, `aesenc(aesenc(aesenc(a, b), KEY), KEY)`.
  The record's group goes into the hash's top byte. A name over 32 bytes, which no generated
  input has, takes the cold `long_letter`.
- **One store per letter.** The 16 bytes go to the mailbox in one store, where the two atomic
  stores took 7 more instructions per record.
- **The entry is 40 bytes.** A slot holds the entry's number and 8 bits of the hash, so a probe
  loads an entry only when they match. The entry holds 64 more bits of the hash, the key, beside
  the 256 group bits. Two names are taken for one only if those 72 bits agree: with about 8,400
  names per worker, a run merges two names with odds of about 1 in 10^13.

Built up on step 39's code (`perf stat`, 20 interleaved runs, against step 39): the hash and the
40-byte entries, instructions -4.3%, cycles -3.0%; with the one store, -6.8% and -3.7%. Kept
apart, as an array of keys and an array of group bits, the marking took 21% more L1 misses and
the runs 0.6% longer; 48-byte entries ran slower than 40-byte ones. Four round keys instead of
one, with `a` and `b` passed through as many rounds, took five instructions and a table of nine
lines, and gained nothing for names that no adversary chose.

Local, against step 40 (`ab.py`, 41 interleaved runs on the P-cores, four rounds; the first three
with a browser busy on one P-core, so the minimums are the steadier figures): medians 25.63,
27.68, 24.22 and 24.81 → 24.69, 25.67, 23.10 and 23.68 ms (-3.7%, -7.3%, -4.6% and -4.6%),
minimums 24.17, 24.30, 23.87 and 24.32 → 22.92, 23.16, 22.69 and 23.22 ms (-5.2%, -4.7%, -4.9%
and -4.5%). `perf stat`: instructions:u 909.4 → 852.6 M (-6.3%, 274 → 257 per record).
memory.peak, 80 alternating runs: median 12.34 → 10.88 MiB, expected highest of five 12.91 →
11.06 MiB. S 393 → 381. Correctness: the gate, 310 pass, 0 fail; full seeds 1 to 7 and 10 to 12 give
the reference hashes; the fuzz and thread-count suite, 3,000 cases, and the 30 edge-case files
pass; of the wide region sets, w2 gives the reference, and w0 and w1 stop with the error as
before; clippy reports no warnings.

## Step 42: 13 fewer lines

Seven of the cuts leave the machine code as it was (the `.text` section's sha256 is step 41's):

- `region_amount` loads its window from `data` directly and returns the `madd`'s sum as it
  comes, with no temporaries; its two SAFETY comments are one.
- `ALIGN` sets each control byte with one `if`/`else`, the table and its index in one `let`.
- `text_fields_slow` and `region_amount_slow` keep `#[cold]` and drop `#[inline(never)]`, as
  `long_letter` does: LLVM keeps cold functions out of line either way.

Six change it, so they were timed:

- The per-round mailbox reads are one `(0..threads).for_each(…)`.
- A mailbox publishes its end, `sent[to]`, instead of its count, so the reader slices up to it
  with no multiply and subtract.
- The slot and the parsed count are bumped in one assignment.
- `parallel`, used once, is written into `join`.

Local, against step 41 (`ab.py`, 41 interleaved runs on the P-cores): the six together, medians
23.59 and 23.67 → -0.21% and -0.30%, and in a third round 23.69 → 23.62 ms, minimums 23.30 →
23.02 ms. Each alone: from +0.04% to -1.10%. S 381 → 368. Gate: 310 pass, 0 fail; clippy reports
no warnings.

The board ran step 41's code a second time in run-13fb3e0130ce (commit c9d5326, which changed only
PERF.md): 21.49 ms, 11.4 MB and 81.65 points.

## Step 43: no runs of late rounds, and rounds of 128 KiB

An instrumented copy of step 42 took a timestamp as each worker arrived at and left each barrier
(local, P-cores, 21 runs; the machine now runs with turbo on, so a run takes about 14 ms instead
of 23). The waits took 1.87 ms of each worker's 11.6 ms of rounds, 16%, though in half the rounds
the last worker arrived within 2 µs of the first. The rest came from about 20 rounds per run in
which the last worker arrived 90 to 110 µs after the first, and in 18 of them that worker had
slept at the barrier before: 2,000 spins last about 60 µs, a sleep of 50 µs may end 50 µs late
with the default timer slack, and the CPU goes idle, so the sleeper left the barrier about 180 µs
after it arrived, late for the next round, its waiters slept in turn, and the run went on for up
to 20 rounds. About two rounds per run started one fresh, with a worker whose round took 140 µs
instead of 40.

- **Short sleeps.** `main` sets the timer slack to 1 ns (`PR_SET_TIMERSLACK`, which the threads
  inherit), and a waiter that has spun 2,000 times sleeps in steps of 10 µs instead of 50. +1 line.
- **Rounds of 128 KiB.** Step 16 found them flat against 64 KiB, with more memory: a round's
  letters to one owner, about 180 of 32 bytes, took two pages of its mailbox. Since step 41 a
  letter is 16 bytes, so they fill 2.9 KB, part of one page as at 64 KiB. Alone, on step 42's
  barrier, they were slower (15.23 against 14.40 ms, medians of 41 runs): each late round now
  costs more.

`ab.py` against step 42, 41 interleaved runs, medians / minimums: seed 1, 14.86 / 12.95 → 13.11 /
12.00 ms (-11.8%); seed 2, 14.20 / 12.99 → 13.26 / 12.16 ms (-6.6%); in the same way with the
tails printed, seed 1, 14.40 → 13.07 ms (-9.2%), and seed 3, 14.10 → 13.02 ms (-7.7%), p90 17.05 →
13.79 ms. With a busy loop of another program on CPU 5 (15 runs): 38.52 → 31.00 ms. memory.peak,
5 alternating runs: 9.9 to 10.5 MB for step 42 and 10.2 to 10.7 MB for this step; no allocation
changed. S 368 → 369. Correctness: the gate, 310 pass, 0 fail; full seeds 1, 2, 3, 7 and 12 give
the reference hashes, and seed 2 does on 1, 3 and 4 CPUs; clippy reports no warnings.

Dropped, against step 42, 41 interleaved runs unless noted:
- **A longer spin**, `SPINS` 30,000 (about 1 ms) instead of short sleeps: as fast in the medians
  (seed 1, 14.48 → 13.05 ms; seed 3, 14.33 → 13.47 ms; 10,000, 100,000 and 200,000 spins within
  0.2 ms of it), and on the instrumented copy the waits fell from 1.87 to 0.76 ms per worker. But
  the tails grew: 3 runs in 61 took 47 to 96 ms, and with a busy loop on CPU 5, 172.9 ms against
  38.5 ms. While the spinners hold every other CPU, the worker that shares its CPU is not moved.
  With rounds of 128 KiB on top, medians 13.14 and 13.10 ms, the same as this step.

## Step 44: 3 fewer lines

S 369 → 366. Both cuts are in code that runs once per worker or once per round, not per record:

- Step 1 numbers the present country codes with `(present().zip(1..)).for_each(…)` and takes
  `group_count` as `1 + present().count()`, instead of a `for` loop with a counter. The extra
  walk over the 17,576 code bits runs once per worker during setup.
- A round's mailbox ends are stored with one `(sent.iter().enumerate()).for_each(…)`.

Four other cuts came out longer after `rustfmt` (calls and chains over 60 characters are split
one argument per line): the `money` closure on one line, the two `write!`s of an output line as
one, the country-code closure inlined into its `collect`, and the rehash's probe as a `find`.

`ab.py`, full seed 1, turbo off, 41 interleaved runs, medians / minimums, step 43 → step 44:
23.57 / 22.96 → 23.47 / 22.65 ms and 23.47 / 22.75 → 23.60 / 23.03 ms, the same within noise.
The gate passes (310 of 310).

On the same machine with turbo off, step 43 runs at 23.5 ms against step 42's 23.62 ms before it:
the 7 to 12% that step 43 measured with turbo on most likely came from idle states the board's
machine does not reach (its run drew 21.88 ms, step 42's 21.86 and 22.25).

## Step 45: hyperthreads that mark while their sibling parses, and 39 M fewer instructions

A profile of step 44 (P-cores, turbo off, 10 runs of `perf record -e cycles:upp`) put 24 to 26% of
the samples in `NameSet::read`, 43% of them on the compare of the entry's key, which waits on the
entry's load, and 5 to 14% on the barrier's `pause`. The marking runs on all 12 threads at once
after each barrier, so both hyperthreads of a core waited on loads at the same time, and both
parsed at the same time, when the core's ports are the limit (about 1.9 instructions per cycle
each).

- **Marking beside a parse.** The round loop runs once more than there are rounds, and in each
  round a worker marks the letters of the round before, from the other half of the mailboxes:
  before its parse if `sched_getcpu()` is even, after it if odd. Both orders mark round r-1
  between barriers r-1 and r, and nothing posts to that half until barrier r has passed, so
  the result is the same. CPUs 2k and 2k + 1 are the hyperthreads of one core here, so each core
  runs one marking next to one parse at the start and the end of each round. The parity is read
  once per round, so a worker that moves still marks each round once. +6 lines.
- **The text window unchecked.** `byte_mask` takes the 64 bytes as `&[u8; 64]`, and
  `text_fields` reads them without a bounds check, as an `unsafe fn`: the id's stop is at most
  the pad's first byte, or its fourth after a BOM, so `PAD` grows to 128 bytes and the window
  always fits. `text_fields_slow` keeps the check for its further blocks. 16.9 M fewer
  instructions (4.8 per record).
- **The probe unchecked.** `NameSet::read` takes the slot with `get_unchecked`, since the slots
  are never empty, and the entry too, since a slot's number names an entry pushed before it.
  13.0 M fewer instructions (3.7 per letter).
- **Min and max as branches.** A record rarely sets a new min or max, so `merge` stores each only
  when it changes. 9.1 M fewer instructions (2.6 per record), and two stores fewer.

`perf stat -r 10`, full seed 1: instructions:u 851.3 → 812.2 M for the last three, then 800.2 M
with the order of marking and parsing; the profile then has 24% in `read` and 5 to 10% in `pause`.
`ab.py`, turbo off, interleaved runs, medians / minimums, step 44 → step 45: full seed 1, 51
runs, 23.39 / 22.89 → 22.68 / 22.02 ms (-3.0%) and 23.43 / 22.85 → 22.52 / 22.08 ms (-3.9%);
full seed 2, 41 runs, 23.38 / 23.01 → 22.54 / 22.05 ms (-3.6%). The three unchecked reads and
the branches alone: 23.52 → 23.06 ms and 23.48 → 23.08 ms (-2.0%, -1.7%); the order of marking
on top of them: 23.04 → 22.74 ms and 23.08 → 22.82 ms (-1.3%, -1.1%). memory.peak, 5 alternating
runs: 10.3 to 10.6 MB for step 44 and 10.0 to 10.4 MB for this step. S 366 → 376. The gate passes
(310 of 310), full seeds 1 and 2 give the reference hashes, and clippy reports no warnings.

## Step 46: the name set's loop never calls out of it, and `number` on an SSE window

`perf stat` on step 45 counted 801 M instructions, and both hyperthreads of each core stay busy,
so the instruction count decides T (see "where step 45 stands" below). Two cuts, together:

- **`read` without a call in its loop.** `insert` was a call inside the marking loop, so after
  it the slots' pointer, their length (for the mask) and the entries' pointer could have
  changed, and the loop loaded all three again for every letter: 3 loads and a `dec` per letter.
  Now a new name returns from `read` into `insert`, which adds it and calls `read` again on the
  letters from that one on; it finds its entry this time. Both calls are in tail position with at
  most six register arguments, so each is a `jmp` and the stack does not grow. `insert` reads the
  key and the tag from the letter itself, `read` takes the letters as `[AtomicU64; 2]` pairs
  (`mail` slices `boxes`), and it walks them as a slice, `letters = &letters[1..]`, since an
  `enumerate` index or a separate rest took two more instructions per letter and spilled
  `groups`. A letter whose name is in its home slot runs 32 instructions instead of 35; each new
  name pays about 70 for the second entry and its probe, 100k times per run. +1 line.
- **`number` on an SSE window**, as measured on step 45 below: 16 M fewer instructions, the same
  lines. Alone it was within noise (below); with the change above it holds.

`perf stat -r 10`, full seed 1, interleaved: instructions:u 800.6 to 802.0 M (step 45) → 797.1 to 799.4
M (the loop alone) → 773.7 to 775.1 M (both), branch misses 1.84 M throughout. `ab.py`, turbo off,
61 interleaved runs per round, medians / minimums, step 45 → step 46: full seed 1, 23.36 / 22.67
→ 23.07 / 22.21, 23.50 / 22.68 → 23.02 / 22.33, 23.24 / 22.53 → 23.10 / 22.24, 23.28 / 22.86 →
22.92 / 22.33 and 23.42 / 22.35 → 22.95 / 22.33 ms; full seed 2, 23.24 / 22.57 → 22.91 / 21.99,
23.80 / 22.76 → 23.55 / 22.59, 23.33 / 22.72 → 22.96 / 22.50, 24.06 / 22.60 → 24.83 / 22.52 (a
busy round, means 30.8 and 31.8 ms) and 23.45 / 22.76 → 22.95 / 22.25 ms. Nine of ten medians
and all ten minimums lower, typically by 1.5%. The loop alone: 23.30 → 23.19 and 23.30 → 23.15
ms; `number` alone: 23.30 → 23.20 and 23.30 → 23.29 ms (seed 1, same rounds). memory.peak, 3
alternating runs: 10.0 to 10.3 MB for step 45 and 10.1 to 10.5 MB for this step. S 376 → 377. The gate
passes (310 of 310), full seeds 1 and 2 give the reference hashes, and clippy reports no warnings.

## Step 47: a name one slot past home found without a branch

At step 46, 87% of the 1.84 M branch misses per run were the probe's tag compare in
`NameSet::read`, for a name that is not in its home slot. Names are drawn uniformly (spec §2.2),
so no placement can favour the frequent ones. With 8,400 names in 16,384 slots, as a worker holds
at full scale, a simulation of the table puts 74.6% of names in their home slot, 14.6% one past
it and 10.8% further on.

- **Home and next in one read.** The probe reads the home slot and the next as one `u64`, and
  `select_unpredictable` takes the next when the home slot's tag is not the name's. If the
  chosen slot's tag and its entry's key match, that is the entry. Otherwise the probe loop runs
  from the home slot as before, so empty slots and inserts are handled where they were. The
  slots get one more, always empty, so the last slot has a next. Entry 0 holds no name, so a
  slot holds its entry's index (no `- 1`) and an empty slot that is chosen leads to entry 0. Its
  key, 0, matches only a name with tag 0 and key 0, as two names collide, on 72 bits. `read`
  takes the slots and the entries as slices first: through `self` the compiler loaded both, and
  the slots' length, again for every letter. +7 lines.

`perf stat -r 10`, full seed 1, interleaved, step 46 → step 47: branch misses 1.84 → 1.02 M,
instructions:u 774.6 → 778.3 M and 774.7 → 778.7 M, cycles:u 528.1 → 553.2 M and 531.4 → 562.2 M.
cycles:u rose while the wall time fell. One likely reason: it counts the barrier's spins, which
grow when the marking ends sooner. `ab.py`, turbo off, interleaved, medians / minimums: full seed
1, 41 runs, 23.29 / 22.36 → 22.75 / 21.90 ms; 61 runs, 22.87 / 22.26 → 22.59 / 21.75, 22.89 /
22.10 → 22.69 / 22.16, 22.89 / 22.12 → 22.76 / 22.10 and 23.19 / 22.21 → 22.81 / 22.04 ms; full
seed 2, 61 runs, 22.99 / 22.36 → 22.67 / 21.92, 22.97 / 22.37 → 22.65 / 22.09, 23.17 / 22.38 →
22.99 / 21.68 and 23.17 / 22.43 → 22.93 / 22.16 ms. All nine medians are lower, by 0.6% to 2.3%
(about 1.2% typically), and eight of nine minimums. memory.peak, 5 alternating runs:
10.0 to 10.3 MB for step 46 and 9.8 to 10.5 MB for this step. S 377 → 384. The gate passes (310 of
310), full seeds 1 and 2 give the reference hashes, and clippy reports no warnings.

## Step 48: the region table in one pass

A timeline of step 47 (P-cores, turbo off, an instrumented copy, 8 runs, from the start of
`main`): the workers start at 0.53 to 0.73 ms and the last at 0.84 to 1.17 ms, the last ends step 0
of the region table at 0.98 to 1.33 ms, the last ends step 1 at 1.27 to 1.94 ms and the first round
starts at 1.33 to 2.00 ms. Step 1 took 0.25 to 0.6 ms: two scans of the 17,576 code bits with an
atomic load each (`present()`), the second parse of the share's rows and a barrier. After the
last round, the release of the input took 0.51 to 0.66 ms per worker, the entries 0.03 to 0.10 ms, and
the merge, format and write 0.2 ms.

- **One step.** Each worker parses its share of `regions.csv` once and writes each row's group.
  A code's group lives in a shared array of 17,576 `AtomicU8`, 0 until the code's first row is
  read; that row takes the lock, checks again and gives the code the next number. A share holds
  almost every code, and the workers start up to 0.6 ms apart, so the first worker numbers most
  codes and the others take the lock for a few. Groups are no longer in bytewise order, so the
  last worker walks the codes in index order (bytewise) to their groups for the output. Each
  worker keeps all 256 groups a byte can name, so both indexes into them (the merge and the
  marking) are checked by the type and need no `unsafe`. The lock is the one that collects the
  workers' groups at the end. S 384 → 384.

`perf stat -r 10`, full seed 1, interleaved: instructions:u 775.2 → 769.6 M and 800.7 → 787.2 M
(the barrier's spins vary). `ab.py`, turbo off, 61 interleaved runs per round, medians / minimums,
step 47 → step 48: full seed 1, 22.03 / 21.61 → 21.92 / 20.95 and 22.22 / 21.75 → 22.01 / 21.43
ms; full seed 2, 22.19 / 21.37 → 22.03 / 21.23 and 22.34 / 21.72 → 22.08 / 21.64 ms. Before the
lines were trimmed (the same code): seed 1, 22.31 / 21.84 → 22.19 / 21.15 and 22.31 / 21.72 →
21.95 / 21.57 ms; seed 2, 22.24 / 21.67 → 22.03 / 21.67 and 22.20 / 21.72 → 21.96 / 21.57 ms.
All eight medians lower, by 0.5% to 1.6% (about 1.0%). memory.peak, 5 alternating runs:
10.3 to 10.7 MB for step 47 and 10.2 to 10.6 MB for this step. The gate passes (310 of 310), full seeds
1 and 2 give the reference hashes, and clippy reports no warnings.

## Step 49: 23 fewer lines

S 384 → 361, in code that runs once per worker, once per round or off the hot path:

- `region_rows`, called once, is written into the worker, and the shares of `regions.csv` come
  from the closure that cuts `records.csv` into runs: a row starts with a digit, so the first
  line after a cut that starts with a digit or a BOM is a row. Its code is one `fold`. -5.
- `mallopt(M_MMAP_THRESHOLD)` goes. It was there because the regions parse freed its lists and
  glibc then raised its threshold; nothing that large is freed now, and the name sets' entries,
  the mailboxes and the region table are past glibc's default 128 KiB either way. memory.peak,
  20 alternating runs, medians 11.48 → 11.50 MB (in a `bash` wrapper, so about 1 MB above the
  brief's figure), and `ab.py` 22.03 / 22.09 / 22.09 / 22.13 → 21.99 / 22.02 / 21.93 / 22.26 ms.
  -2.
- `map_file` panics with the error and the path instead of returning a `Result` that `main`
  unwrapped, so the mapping's check is one line. -2.
- `ENTRY`, used once, goes (its doc moves to `NameSet::slots`). The rehash finds its empty slot
  with `find`. The round's mailbox ends are stored with one `for_each` over `0..threads`. The
  output line takes its key through `escape_ascii` instead of a separate `extend_from_slice`.
  `values` shuffles in two statements instead of one split over four lines, `prefix_xor` is two
  `unsafe` expressions, `number` takes its length with `trailing_ones`, and its loop for long
  fields is a `position` and a `fold`. `region_amount` tests the sign inline, and
  `text_fields_slow` takes the parity at a block's start from its count of quotes so far. -14.
- `too_many_lines` is allowed in `Cargo.toml`: `join` went past clippy's 100 lines.

`perf stat -r 10`, full seed 1, interleaved: instructions:u 762.2, 762.6, 763.2 and 762.1 M at
step 48, 761.9 and 763.7 M here. `ab.py`, turbo off, 61 interleaved runs per round, medians /
minimums, step 48 → step 49: full seed 1, 21.99 / 21.51 → 21.93 / 21.42, 22.01 / 21.58 → 22.00 /
21.46 and 22.01 / 21.49 → 21.96 / 21.46 ms; full seed 2, 22.08 / 21.60 → 22.13 / 21.64 and
22.02 / 21.50 → 22.01 / 21.57 ms, the same within noise. memory.peak, 20 alternating runs,
medians 11.56 → 11.54 MB. The gate passes (310 of 310), and clippy reports no warnings.

Measured and not kept:
- **The two tails of `text_fields` as one**: the slow path returned the two ends with their
  quote counts and the fast path's tail computed the lengths for both. -3 lines, but
  instructions:u 762.0 → 794.9 M (+4.3%).
- **`number`'s loop for long fields kept as it was**, with the other cuts: the worker's function
  is 7,702 bytes against 8,837 with the `fold`, which LLVM unrolls, yet `ab.py` measured 22.11,
  22.13 and 22.12 ms against 22.01, 21.99 and 22.04 ms with it (61 runs, interleaved with step
  48 at 22.03, 21.92 and 22.00 ms): the layout of the hot loop moved.

## Step 50: groups built on the heap, and main as worker 0

What is live at the peak, the end of the last round (step 49's code, full seed 1, P-cores, an
instrumented copy that reads `/proc/self/pagemap` and `memory.stat` there; 4 KiB pages):

| Block | Pages | MB |
|---|---|---|
| name-set entries, 12 × ~8,400 names × 40 bytes | 988 | 4.05 |
| mailbox pages, 2 × 144 mailboxes × 1 page | 288 | 1.18 |
| region table, 1 byte per id | 245 | 1.00 |
| name-set slots, 12 × 16,385 × 4 bytes | 204 | 0.84 |
| thread stacks, 12 × 6 pages | 72 | 0.29 |
| groups, 12 × 16 KiB | 48 | 0.20 |
| heap, binary, main's stack, `group_of` | ~15 | 0.06 |
| page tables (memory.stat) | | 0.72 |
| kernel stacks, 13 threads | | 0.21 |
| slab | | 0.13 |
| memory.current less all of the above | 540 to 610 | 2.2 to 2.5 |

The last row is memcg's per-CPU charge stock: a CPU that charges a page takes 64 at a time
from the counter and keeps the rest for its next charges, so each of the 12 CPUs holds 0 to 63
pages that memory.peak counts and no page uses (plus the `bash` wrapper's own, in this
measurement). Of the input's page tables, about 97 pages are the 202 MB mapping.

- **Groups on the heap.** The worker's frame was 16.9 KB: `Box::new([EMPTY; 256])` built the
  16 KiB of groups on the stack and copied them into the box, and the stack probes touched every
  page of the frame, which stayed mapped. `vec![EMPTY; 256]` builds them in place; the frame is
  664 bytes and each stack touches 2 pages instead of 6. anon 7.96 → 7.77 MB in the same dump.
  +1 line.
- **Main as worker 0.** `main` spawned all 12 workers and slept in the scope. Now it spawns 11
  and runs worker 0: one spawn fewer before the first round and one thread fewer. +3 lines.

Both alone, `ab.py`, 61 interleaved runs, medians: groups on the heap 22.03 → 22.02, 22.04 →
22.02 and 22.08 → 22.00 ms; main as worker 0 on top of it 22.16 → 21.96, 22.28 → 22.33, 22.04 →
21.94, 22.11 → 21.89, 22.07 → 21.89, 22.03 → 21.91 ms, and on full seed 2 22.14 → 22.05 ms.

Together, step 49 → step 50, medians / minimums: full seed 1, 22.08 / 21.58 → 21.88 / 21.53,
22.04 / 21.44 → 21.98 / 21.51 and 22.05 / 21.66 → 21.93 / 21.56 ms; full seed 2, 22.00 / 21.53 →
21.95 / 21.57 and 22.16 / 21.58 → 21.95 / 21.57 ms (-0.6%). instructions:u 761.9 and 762.5 M →
762.2 and 763.0 M. memory.peak, the brief's `systemd-run`, 8 alternating runs: 10.2 to 10.6 MiB
(median 10.4) → 9.9 to 10.7 MiB (median 10.15); in the `bash` wrapper, 20 runs, medians 11.52 →
11.46 MB. S 361 → 365. The gate passes (310 of 310), and clippy reports no warnings.

Costed and not built, for memory:
- **36-byte entries**, a 32-bit key: the rehash places a slot by its entry's key, so the key
  must hold the home bits, and 32 bits of key with the 8-bit tag leave 40 bits to tell two names
  apart, about 1 in 2,600 runs merging two names at full scale. -0.4 MB.
- **Mailboxes packed below a page**: a mailbox holds the worst case, every record of a round to
  one owner, so they are a page or more apart and each touches a page per half. With 8-byte
  letters (56 bits of hash) and 2 KiB boxes, with an overflow area for the worst case, the
  mailboxes would take 144 pages instead of 288, -0.59 MB, for a test per post and about 20
  lines.
- **The input's page tables freed during the rounds** by unmapping each 2 MB the worker has
  parsed: about -0.3 MB, but `munmap` takes the address space's lock for writing, which the
  workers' unmaps then take one at a time (step 38 measured 0.7 ms for the name sets'), and
  the input would need one mapping per run.
- **The charge stock**: memcg drains it only under reclaim, at a limit or when another cgroup
  charges on that CPU, none of which the program controls.

Costed and not built, for speed: **the id and the region's amount in one 256-bit chain**
(`vinserti128` of the two windows, then one subtract, shuffle and multiply-add chain for both).
It saves 3 to 5 of the ~205 instructions a record takes (3.72 M records, 762 M instructions),
for 10 to 15 lines and a slow path that must undo the pairing when either field is long or
carries the byte-order mark. At 0.011 points a line it must win more than 1.2% of T to pay,
and at most 2.5% of instructions go.

## Step 51: one worker fewer than the CPUs

Two of eleven board runs of steps 48 to 50 took 47 to 49 ms instead of about 20. Locally, with a
busy loop pinned to one of the twelve CPUs, step 50 takes 57.6 ms (median of 31, full seed 1)
instead of 21.9: the worker on that CPU gets half of it, every one of the 129 rounds waits for that
worker at the barrier, and the scheduler leaves it where it is because it is cache-hot. The same
busy loop costs A, B and D 17 to 28%, since they meet at a barrier only a few times.

The final score is the mean of the middle three of five runs, and a batch is re-run only when they
spread by more than 50 ms, which a 47 ms run does not reach. At two slow runs in eleven, about one
batch in five would hold two, and the mean of the middle three would then be about 29 ms, 4 points
less. With one worker fewer, a busy CPU leaves a spare one:

| Binary (full seed 1, 31 runs each) | No busy CPU, median | One busy CPU, median |
|---|---|---|
| Step 50, 12 workers | 21.9 ms (2 of 31 runs above 1.5x) | 57.6 ms |
| Step 51, 11 workers | 24.0 ms (1 of 31) | 23.7 ms |

`ab.py`, 41 interleaved runs on full seed 1, step 50 against step 51: medians 22.63 against 23.95
ms (+5.8%, about 0.6 points), means 33.79 against 24.68 ms, because step 50 had slow runs under the
machine's own load. The cost is paid on every run. It buys out a one-in-five chance of about 4
points.

## Step 52: 8 fewer lines

S 365 → 357 (Rust 359 → 352, the Dockerfile 6 → 5), in code outside the record loop or with the
same machine code inside it:

- The build stage runs `RUN --mount=type=bind,target=/src cd /src && cargo build …`, as tracks A
  and D do, instead of `WORKDIR /src` and then the mount. Both stages stay. The image's `/app/join`
  has the local build's sha256. -1.
- The three `use` lines are two: `arch::x86_64::*` joins the `std::{…}` list, and `Duration`, used
  once, is written `std::time::Duration` there. -1.
- Both `prctl` calls set their option to 1, so they are one `map` over `[41, 29]`, with one
  comment for the two options. -1.
- `BOM` and `SPINS`, each used in one place, are written there: the record loop tests for `0xEF`
  and parses the id again 3 bytes on, and the barrier sleeps after 2,000 spins, its counter typed
  `usize` as `SPINS` made it. The comments keep what their docs said. -2.
- `half` and `odd` are set in one statement, and so are `names` and `sent`. -2.
- `text_fields` names the offsets of its two commas `name` and `note`, as `text_fields_slow`
  does, which makes the returned tuple short enough to take the name's range directly. -1.

Every cut but one leaves the machine code as it was. `names` and `sent` in one statement change
the worker's code (9,250 → 9,328 bytes) but not what it runs: with one worker (`taskset -c 2`),
instructions:u 753,563,908 at step 51 and 753,563,915 here. `perf stat -r 20` on the P-cores,
interleaved: instructions:u 765.1, 765.1 and 765.1 M → 765.2, 765.0 and 765.1 M; cycles:u 481.5,
480.7 and 481.5 M → 481.7, 480.1 and 480.5 M. `ab.py`, turbo off, 61 interleaved runs, medians /
minimums, step 51 → step 52: full seed 1, 23.03 / 22.49 → 23.02 / 22.55 and 23.08 / 22.28 →
23.04 / 22.54 ms; full seed 2, 23.04 / 22.67 → 23.09 / 22.62 and 23.07 / 22.66 → 23.11 / 22.73 ms.
memory.peak, the brief's `systemd-run`, 5 alternating runs: medians 9.7 → 9.7 MiB on full seeds 1
and 3; no allocation changed. The gate passes (310 of 310), and clippy reports no warnings.

Measured and not kept:
- **Each output line as one `format!`**, the lines collected into a `String`: -2 lines, but with
  one worker 753.56 → 754.12 M instructions (+551 k per run, about 2,300 per line, since each
  `format!` starts with no capacity and grows), all of it in the last worker's tail after the
  rounds.
- **`text_fields` returning the note's comma**, the caller adding 1: -1 line, instructions:u
  765.4 → 768.5 M (+0.4%).
- **`region_amount_slow` written into `region_amount`'s branch** behind `std::hint::cold_path()`:
  -3 lines, but the worker grew from 9.3 to 10.3 KB and instructions:u went 765.4 → 778.4 M
  (+1.7%).
- **`long_letter` written into `name_letter`**, not built: `name_letter` would then call itself,
  and LLVM does not inline a recursive function, here into the record loop.
- Left for clarity: `NameSet::new` written into the worker (-2), `main` and `join` as one function
  (-2), the `ALIGN` loop's `i += 1` folded into a tuple assignment (-1).

## Step 53: a worker on every CPU again

Step 51 gave up a worker so that a CPU taken by another program would not halve one worker and
every round. That load came from the shared live board and from this machine's other programs.
The board README says final scoring is "five runs on the hidden holdout seed, with the box to
itself", one run at a time, and the highest of the five is dropped. Two slow runs in one batch of
five on an idle box are unlikely, while the spare CPU costs every run.

`ab.py`, 61 interleaved runs, step 52 against step 53, medians / minimums: full seed 12, 23.00 /
22.68 → 21.44 / 21.01 ms (-6.8%); full seed 1, 22.97 / 22.40 → 21.32 / 20.70 ms (-7.2%), about
+0.75 points. One run of step 53 on seed 12 was slow under this machine's own load (mean 26.90
ms), the kind of run final scoring drops. S 357 → 357. Gate: 310 pass, 0 fail.

## Step 54: 9 fewer lines

S 357 → 348 (Rust 352 → 343), with the record loop's machine code unchanged:

- `NameSet::read` tests whether a slot's entry is the name's in one closure, `hit`, which the home
  slot's test and the probe loop share; the loop breaks with the entry's index, and one unchecked
  `get_unchecked_mut` after it takes the entry. The slots' mask is set once with the slices, and the
  tag and the group are bytes 14 and 15 of `high.to_le_bytes()`, as the docs number them. -3. The
  function's machine code has the same 78 instructions, one `shr` moved.
- `name_letter` takes the length in a `let len @ 0..=32 = … else` that calls `long_letter`, where an
  `if` returned: `cmp $0x21; jae` for `cmp $0x20; ja`. -1.
- `text_fields_slow` XORs the parity of the quotes before a block into bit 0 of its quote mask,
  which carries it into every bit of the prefix XOR, instead of XORing all ones in after it: one
  instruction fewer on that cold path. -1.
- `text_fields` calls `before(name)` and `before(note)` in the lengths, without naming the counts;
  the compiler computes each once, as before. -1.
- The batch writes a record's group through `top[1].as_ptr()`, byte 7 of the letter's second word,
  instead of casting the letter's reference to a byte pointer first. Same machine code. -1.
- The mailboxes are one `Box::leak` split by `as_chunks`, with no `letters` binding: nothing is
  freed anyway. -1.
- `main` reads the data directory inside the closure that maps the files (`env::args` imported in
  place of `fs::write`, now written in full at its one use). -1.

Every function has the same instruction count as at step 53, compared function by function with
`objdump`, except `main` (191 → 337 instructions: `args` is parsed twice, once per run) and
`text_fields_slow` (122 → 121); the worker's 1,892 differ only in register choices. `perf stat -r 5`, interleaved, instructions:u, step 53 → step 54: full seed 1, 762.28,
762.27 and 762.19 M → 762.10, 761.95 and 761.93 M; full seed 2, 762.37, 762.51 and 762.16 M →
762.00, 762.06 and 764.38 M (that run also took 531.7 M cycles against 485 to 493 M for the others, a
busy CPU). cycles:u 488.6, 488.1 and 486.3 M → 487.0, 483.6 and 483.6 M on seed 1. `ab.py`, turbo
off, 21 interleaved runs, medians / minimums: full seed 1, 21.89 / 21.39 → 21.59 / 21.26 and 21.84 /
21.43 → 22.05 / 21.37 ms; full seed 2, 21.94 / 21.36 → 21.93 / 21.37 ms, the same within noise. No
allocation changed. The gate passes (310 of 310), and clippy reports no warnings.

On the board: step 54's first run, run-83d1167e1735, came on a slow box, and the next push replaced
its queued rerun. The C code has not changed since, so every later C run measures step 54:
run-dc3871163ade (pushed with 0491b7f) took 19.42 ms, 11.36 MB, 83.07 points, and run-a69fa3d057dc
(47858fc) 19.63 ms. One later run, run-e75aab520ba9 (61a6ab7), took 53.37 ms while A, B and D in
the same push ran at their usual times (16.87, 93.65 and 121.77 ms): the busy CPU of step 51, which
final scoring, with the box to itself, should not see (step 53).

Measured and not kept:
- **`text_fields` with the note's comma from `let note @ 0..64 = … else`**: -1 line, but the test
  became `tzcnt; cmp $0x40; jae` after the `blsr` whose flags the `je` used, one instruction more
  per record.
- **The posted letter's slot read through `sent`'s reference** at each use, without `slot`: -1
  line, but the worker grew from 1,892 to 1,907 instructions (the slot is loaded again after the
  letter's store).
- **The input's prefetch in one statement**: rustfmt splits it over three lines (104 columns).
- Left for clarity: `format_output` written into the last worker's tail (-3), `map_file` as a
  closure in `main` (-1), the rehash without its filter of empty slots, which would place each as
  entry 0 into an empty slot, a no-op (-1).

## Experiments that did not stay

- **One malloc arena for all threads** (after step 53; local, seed 1): `mallopt(M_ARENA_MAX, 1)`
  at startup. `/proc/<pid>/smaps` at the peak showed twelve 132 KiB arena heaps, one per worker,
  at 84 KiB resident each, 1 MB in all, beside a 116 KiB main heap. With one arena the same pages
  were touched in the main heap, so the 84 KiB are the workers' own data and not the arenas' slack:
  memory.peak over five alternating `systemd-run`s, medians 10.3 → 10.3 MB. `ab.py`, 41 interleaved
  runs: 21.36 → 24.09 ms (+12.8%), the workers' allocations serialized on one lock. Not kept.
- **Rounds cut into chunks that the workers claim, with 12 workers again** (after step 51; local,
  P-cores, turbo off). Each run's 128 KiB share of a round became 8 chunks of 16 KiB, claimed
  from per-run atomic counters, own run first; a worker took another run's chunk only while each
  mailbox it writes to had room for one more chunk. With a busy loop pinned to CPU 3 the kernel
  put two of our workers on one other CPU, and a 60 µs barrier spin starved the second one, so a
  worker that shared its CPU with another worker marked first, yielded after each chunk and
  yielded instead of spinning. `ab.py`, 61 runs, medians: 21.76 and 22.16 ms on seeds 1 and 2,
  against 21.78 and 22.12 for step 50 and 23.49 and 23.78 for step 51. With the busy CPU: 25.3 ms
  against 22.1 quiet (+14.5%), where step 51 stays flat and step 50 takes 57.3. It costs 31 lines
  and 0.6 MB, so it nets about +0.16 points on a quiet run and about +0.04 once busy runs are
  counted. A worker that the scheduler leaves on the same CPU as another program gets no special
  handling, a placement that never happened locally but would bring back step 50's slowdown.
  Chunks with the spinning barrier took 48.5 to 50.0 ms under the busy CPU; 4 or 16 chunks per
  round 28.2 and 28.9 against 25.5 for 8; handing a name set to the worker on the same CPU 25.5,
  1% slower quiet, and one wrong hand-off in 20 runs.
- **Where step 50 stands** (local, P-cores, turbo off, an instrumented copy with a timestamp at
  each phase of each worker, 21 runs, medians, ms from the harness's spawn of the process).
  `main` starts at 1.2 to 1.7 ms (an empty static program: 1.6 ms) and `join` 0.09 ms later.
  The first spawn returns 0.35 ms after `join` starts (the region table's `MADV_POPULATE_WRITE`
  and the cuts), the last 0.73 ms after; the workers start over 0.25 to 0.3 ms, and the first
  round starts when the last of them has parsed its share of the rows, 0.95 to 1.0 ms after
  `join` starts. The 129 rounds take 18.2 ms: per worker 13.1 ms of parse, 3.9 ms of marking
  and 0.9 to 1.0 ms at the barriers; in half the rounds the arrivals spread over less than
  5 µs, and the barrier lets everyone go within 0.3 µs of the last arrival. Then the release,
  0.53 to 0.6 ms, the entries 0.07 ms, the merge 0.04, the format 0.13 and the write 0.04 ms.
  The process is gone 0.7 ms after the write, where an empty static program exits in 0.18 ms;
  an empty program with 11 parked threads takes 0.42 ms, and one that maps and touches the
  input and gives it back with `MADV_DONTNEED` 0.20 ms. In the kernel profile (`cycles:k`, 10
  runs), `madvise` is 20% of kernel cycles, the barrier's sleeps 15% and the exit 0.7%.
- **The region rows in shares taken in turn** (on step 50's code): 128 shares of
  `regions.csv`, each worker taking the next from an atomic counter and cutting its share
  itself, so that the workers that start first parse more of the rows and the last to start
  finds them done. With the 96 cuts made by `main` before the spawns, its page faults on
  `regions.csv` made the spawns 0.1 ms slower and ate the gain. The instrumented copy put the
  first round 1.005 → 0.866 ms after `join` starts. `ab.py`, 61 interleaved runs, medians:
  seed 1, 21.95 → 21.83, 21.96 → 21.87 and 21.96 → 21.83 ms; seed 2, 22.06 → 21.98, 22.04 →
  21.97 and 21.99 → 21.95 ms; two more sets of six rounds, three- and four-way, -0.49% and
  -0.31%. About -0.4% for +3 lines (S 365 → 368), worth +0.01 points at most. 96 or 48 shares:
  the same within noise. The gate passes (310 of 310).
- **Workers that end their own thread** with the `exit` syscall instead of parking, so the
  last worker's exit kills no thread (on step 50's code): the time from the write to the end
  fell 0.70 → 0.62 ms in the instrumented copy, but `ab.py`, six rounds of 61 runs on seeds 1
  and 2, medians averaged 22.04 ms for step 50 and 22.06 ms for this. +2 lines.
- **The input given back in shares that all workers take**, so that the last worker's merge,
  format and write (0.22 ms) overlap the others giving back its run: giving back pages that
  another core faulted in is much slower, and the last share was given back 1.64 ms after the
  last round against 0.6 ms. The merge and format also slowed, 0.17 → 0.28 ms, beside the
  siblings' `madvise`.
- **A long spin at the first barrier only** (50 × `SPINS`), so that the workers that start
  first do not sleep through the last arrival: `ab.py`, six rounds of 61 runs on seeds 1 and
  2, 22.01 → 22.05 ms (medians averaged).
- **`pread` into a small buffer instead of the mapping**, costed with a test program (12
  threads on the P-cores, each streaming its twelfth of `records.csv`, 5 runs): through the
  mapping 5.1 ms of faults and reads plus 0.6 to 0.7 ms of `MADV_DONTNEED`, through `pread`
  into a 16 to 128 KiB buffer 5.7 to 6.0 ms with nothing to give back. A wash, and the buffers
  would add 0.2 to 1.5 MB of memory.

- **The input mapped in up front** (local, turbo off, on step 47's code): each worker called
  `madvise(MADV_POPULATE_READ)` on its run before the region table, so that the rounds would
  not fault. Page faults 4,724 → 1,654, but cycles:k 59.0 → 63.5 M and `ab.py`, 41 interleaved
  runs, medians 22.11 → 27.23 ms (+23%): the populate is slower per page than the faults it
  replaces, and it delays the first round.
- **Where step 47 stands** (P-cores, turbo off, `perf record -e instructions:upp` and
  `cycles:k`, an instrumented copy for the rounds). Per record: about 150 instructions of parse
  and post (id 21, `text_fields` 44, the hash 14, `region_amount` 36, the post, the batch and the
  loop about 35), 28 of merge, and 33 per letter in `NameSet::read` for a name at home; 775 M in
  all. cycles:k is 10% of all cycles: 55% of it the input's page faults (`filemap_map_pages`,
  `next_uptodate_folio`), about 1.1 ms per worker inside the rounds, and 18% the release after
  them. In the rounds a worker works 95% of the span on average and the busiest 97%, so the
  barriers cost about 3% of jitter and 1.5% for the owner of the empty name, which gets 12% more
  letters. Costed and not built: the marking four letters at a time with `vpgatherqq` for the
  slot pairs and the keys, about 8 instructions fewer per letter (a third of the groups of four
  hold a name past home + 1), for about 35 lines; the id's value and `region_amount` from one
  256-bit shuffle and multiply-add chain, about 5 fewer per record; a third set of mailboxes, so
  a worker can run a round ahead, +0.58 MB for at most the 3% of jitter.

- **Other ways to make the not-home case cheaper, costed before step 47** (simulation of 8,400
  names in 16,384 slots, as one worker at full scale; none built):
  - Robin Hood insertion: 64.0% at home, 24.6% one past it and 11.4% further on, against linear
    probing's 74.6%, 14.6% and 10.8%. That is more names off home, and no fewer beyond the next
    slot.
  - Aligned buckets of two slots, with overflow into the next bucket: 12.7% leave the home
    bucket, against 10.8% beyond home + 1 for the unaligned pair of step 47.
  - A "displaced" bit in the home slot: whether a name is home stays as unpredictable as before
    (a quarter are not), so the bit only moves the missed branch.
  - Starting the probe one slot past home when the home slot's tag differs, branch-free, with
    no separate hit path: skipping a home slot that is empty makes it linear probing from home
    + 1, with the same misses, and testing that the home slot is taken costs 7 to 8 instructions
    per letter.
  - A dense tag array matched 16 slots at a time: at least 16 KB more per worker beside the
    32-bit indexes. Step 32's eight-slot AVX2 compare already cost 32 M instructions for no gain.
- **The record's sums as one vector in the batch** (local, turbo off, on step 46's loop): the
  batch held `[1, id, cents, name_len]` as one `__m256i` from `_mm256_set_epi64x`, so the merge
  adds it with one `vpaddq` from memory and no vector built from four loads. Instructions 797.3
  → 794.1 M only: the vector is now built at the parse, as two halves that the compiler stores
  with two 16-byte stores, and the merge's 32-byte load cannot be forwarded from them
  (`ld_blocks.store_forward` 3.5 k → 1.02 M per run). As eight `i64` in the batch, stored one by
  one, the same forwarding fails, and it loses the knowledge that word 5 is 0.
- **The probe's tag compared first, with an unused entry 0** (local, turbo off, on step 46's
  code): a slot holds its entry's index, entry 0 is a placeholder with key 0, and the probe tests
  the tag and the key before the empty slot, which drops a `jmp`, an `add` and a move from a hit.
  764.2 against 773.7 M instructions, but branch misses 1.84 → 1.89 M and `ab.py`, 61 interleaved
  runs, medians: 23.10 → 23.12, 22.92 → 22.96 and 22.96 → 23.10 ms. +1 line.
- **Prefetching ahead in `NameSet::read`** (local, turbo off, on step 44's code), now that the
  entry's load holds 43% of its samples: at letter i, the slot of letter i + 2k prefetched and the
  entry of the slot of letter i + k (its first probe) prefetched, k = 4, 8 or 16, with the key's
  line or the entry's first. `ab.py`, 25 interleaved runs, medians: 23.37 → 23.74, 23.78, 23.91
  and 23.85 ms. For k = 8: L1 misses 11.38 → 6.63 M, but instructions 851.4 → 928.5 M (22 per
  letter, the bounds checks of the reads ahead) and cycles 537.4 → 552.6 M. The entry prefetch
  alone, 6 letters ahead: 23.45 → 23.48 ms (31 runs). Step 9's stage 4 found the same with
  64-byte entries.
- **The id's digits summed without parsing them** (local, turbo off, on step 45's unchecked reads
  and branches): the id's word, shifted so its last digit is the top byte, went into the batch,
  and the merge added its eight bytes, widened by `vpmovzxbd`, into four more words of the group,
  as eight 32-bit sums of the digits at each place, joined into `id_sum` only for the output. That
  skips the id's three multiplies. Instructions 812.2 → 809.0 M only, since the batch, the
  96-byte groups and the slow path's value took back most of them, and cycles 520.8 → 532.4 M
  and 521.9 → 528.3 M (`perf stat -r 10`, interleaved).
- **The letter kept in a vector register** (local, turbo off, on step 45's code): `name_letter`
  returned the `__m128i`, the owner came from `_mm_extract_epi32`, and the store took the
  register, where the `[u64; 2]` went through the stack. 800.6 → 793.6 M instructions, but
  `ab.py`, 51 interleaved runs, twice: 22.56 → 22.74 ms and 22.59 → 22.89 ms. +2 lines.
- **Only the slot read unchecked** in the probe, with the entry's bounds check kept (one line
  fewer): 793.6 → 803.2 M instructions on the build above.
- **Where step 45 stands** (P-cores, turbo off, `perf stat -M TopdownL1/L2`, 10 runs): retiring
  47%, core bound 22%, branch mispredicts 10%, fetch latency 10%, memory bound 6%. Of the 1.84 M
  branch misses, 87% are the probe's tag compare in `NameSet::read` (a name not in its home
  slot, at a load of 0.51), and in the record loop the most cycle samples sit on the region
  byte's prefetch, which waits on the region id's multiply-add chain.
- **The name sets' slots at a quarter load** (local, turbo off, on step 45's code): twice the
  slots, 32,768 per worker at full scale. Branch misses 1.84 → 0.81 M, instructions 800.7 →
  792.4 M, cycles 522.9 → 520.6 M and 526.5 → 517.8 M (`perf stat -r 10`, interleaved), but
  memory.peak 9.9 to 10.2 → 10.6 to 10.9 MB (4 alternating runs): +0.7 MB costs about 0.25 points,
  which 1% of T does not pay for.
- **`number` on an SSE window** (local, turbo off, on step 45's code; kept in step 46 with the
  name set's loop): the id, and every other
  field `number` reads, parsed as `region_amount` parses the region id, a compare with `/`, its
  mask's first zero for the length, and the `pshufb` and multiply-adds of step 35 shared with
  `region_amount`, instead of three multiplies of one word. The same lines (S 376), 800.8 →
  784.5 M instructions, and the gate passes, but `ab.py` on a busy machine (other programs on
  the P-cores, means up to 33 ms), interleaved, medians: seed 1, 23.43 → 23.28, 23.07 → 23.05
  (51 runs) and 23.15 → 22.97 ms (61 runs); seed 2, 23.35 → 23.54 ms (61 runs). Within noise,
  probably because the vector compare and `vpmovmskb` put the id's comma a few cycles further
  along the record's chain, and the multiply-adds share their ports with the text masks and the
  hash.
  As a separate `id` function beside the SWAR `number`: 14 more lines for the same instructions.

- **A structural index of the records** (local, on step 32's code), to break the chain between
  records: each record's start waits on the id's comma, the name's and the note's closing
  quotes, the region id, the sign, the amount's dot and the CR test, about 80 cycles. Two
  shapes, both with the reference hashes on full seeds 1 and 2 and the 300 tiny seeds, and both
  slower:
  - Two stages per 4 KiB of a round, after simdjson. AVX2 compares of each 64 bytes with `"`,
    `,` and LF, the bytes inside quotes as the prefix XOR of the quote mask (one PCLMULQDQ
    against all ones, carried from block to block), and every comma and LF outside them
    written to a buffer, eight positions at a time with no branch per position. The record
    loop then took five positions per record, so no record waited on the one before. `ab.py`,
    40 interleaved runs, two rounds, medians / minimums 32.73 / 32.15 → 37.02 / 36.20 ms and
    32.99 / 32.09 → 37.04 / 36.00 ms (+12 to 13%). `perf stat -r 10` on the P-cores, twice:
    instructions 1.414 → 1.941 G, IPC per thread 1.75 to 1.87 → 2.15 to 2.20, branch misses
    2.87 → 1.93 M. The record loop was not tuned (each field's quotes were counted by a loop),
    but the index alone took about 90 instructions per 64-byte block, about 85 per record.
  - Each record's end from one compare: step 32's parser, plus one 32-byte compare with LF
    right after the note, which gives the next record's start, so the region id, the sign, the
    amount and the CR test leave the chain; the amount's dot stays the fallback when no LF lies
    in those 32 bytes. 33.10 / 32.35 → 33.78 / 33.17 ms and 33.18 / 31.80 → 34.12 / 33.13 ms
    (+2.1 to 2.8%); instructions 1.416 → 1.450 G, 10 more per record; IPC per thread
    1.82 → 1.80; branch misses 2.87 → 2.92 M.
  - Without hyperthreads, 6 threads on CPUs 0, 2, …, 10 and 30 interleaved runs, both win:
    45.99 / 44.19 ms for step 32, 45.00 / 42.76 ms for the two stages and 43.70 / 41.59 ms for
    the compare. A thread alone does wait on the chain, but with 12 threads its sibling fills
    those cycles, two threads at 1.75 to 1.87 instructions per cycle each, so the instruction
    count decides T.
  - The full per-record window, masks over the 128 bytes from each record's start with the
    k-th comma or LF taken by `pdep`, was not built: by an estimate from its compares, mask
    moves and 128-bit bit walks it runs 20 to 30 more instructions per record than step 32, more
    than the compare that already lost. On full seed 1, 41% of records are longer than 64 bytes
    and 0.06% longer than 96. A 64-byte mask misses the note's closing quote for 11.2% of
    records when it starts at the record and for 2.2% when it starts at the name, as step 28's
    does. The region id, the amount and the terminator never take more than 17 bytes.
- **48-byte name-set entries** (local, on step 32's code): the key's words for names of up to
  15 bytes and the 32 bytes of group bits in 48 bytes, with longer names in a side list. M fell
  on every seed (10 seeds × 11 rounds, per-seed medians -0.33 to -1.02 MB, mean -0.69 MB), but
  `ab.py` went from 32.88 / 32.04 to 35.28 / 34.47 ms (median / minimum, +7.3%), with
  instructions 1,415 → 1,512 M, branch misses 2.87 → 3.44 M and L1 misses 11.2 → 13.1 M. A
  48-byte stride puts half the entries across two cache lines. Step 32's 64-byte entries moved
  16 bytes off their alignment, so that every entry crosses two lines, cost the same 1.8 M
  extra L1 misses and +2.9% T with fewer instructions, so most of the loss is the layout itself.
- **Eight slots probed at once** (local, on step 32's code): the tags of eight slots compared
  with one AVX2 compare and a `tzcnt`. Branch misses fell from 2.87 to 1.34 to 1.40 M, but no
  variant saved cycles. Unaligned groups with a mirrored tail measured 32.95 → 32.69 ms with
  32 M more instructions and 1.3 M more L1 misses. Aligned groups measured 33.07 → 33.10 ms,
  and with the hit's entry taken from the register, cycles went 736.3 → 735.8 M. Peeking at a
  pair of slots with `select_unpredictable` cut branch misses to 2.14 M and cost 2.6% more
  cycles. The mark loop's mispredicts are partly hidden by the hyperthread sibling, so removing
  them gains less than their count suggests.
- **Loops on 64-byte boundaries** (`-C llvm-args=-align-loops=64`, as track B builds; local,
  on step 32's code): no gain. `ab.py`, 40 interleaved runs, two rounds, medians / minimums
  33.01 / 32.02 → 33.00 / 31.73 ms and 33.03 / 31.96 → 33.09 / 31.85 ms. The output hash is
  unchanged.
- **`sent[to]` read once per record** (local, on step 32's code): the four atomic stores of a
  letter make the compiler reload `sent[to]` five times, each with a bounds check. Reading it
  into a local first measured slower: medians 33.01 → 33.37 and 33.03 → 33.89 ms (`ab.py`, 40
  interleaved runs, two rounds). Step 34 reads it once after all, where on top of its other
  changes it cut cycles:u by 1.9%.
- **PGO** (local, turbo off, on step 27's code): a profile from three runs on each of full seeds
  2 to 4, passed as `-C profile-use`, with every function matching it. `ab.py`, 40 interleaved runs,
  medians / minimums: 36.35 / 35.39 → 37.71 / 36.69 ms and 36.27 / 35.64 → 37.39 / 36.79 ms on
  seed 1; 37.38 / 36.22 → 38.55 / 37.47 ms and 37.14 / 36.35 → 38.39 / 37.31 ms on seed 10, which
  it was not trained on. `perf stat -r 5`: instructions:u the same, 1.535 G; cycles:u 0.841 → 0.875 G.
- **The empty name counted by its posters** (local, on step 25's code). Every record with an
  empty name posts its letter to one owner, which then gets 11% more letters than the others
  (306k against 273 to 277k at full scale) and marks for 0.4 to 0.5 ms longer. Each poster instead kept
  one bit per country for the empty name, ORed together at the end. Not posting those letters
  measured the same, 37.33 against 37.16 ms (`ab.py`, 40 interleaved runs, medians), and keeping
  the bits in the batch loop, with or without a branch, cost 2.2 to 2.3 ms through the loop's
  register allocation.

- **The name slots written at allocation** (`[0].repeat(n)` instead of `vec![0; n]`, local, turbo
  off, on step 17), after track B found that a first load from a calloc'd page maps the shared
  zero page and the store after it interrupts every core: `ab.py`, 40 interleaved runs, medians
  40.16 → 40.18 ms on seed 1 and 39.74 → 39.80 ms on seed 2; page faults 6,350 → 6,351, kernel
  cycles 64.8 → 62.9 to 64.2 M. The slots do not fault that way here.
- **Batched table lookups** (local only): each thread parsed 32 records, then loaded every
  record's region entry and hash slot, then found every name's entry, so the cache misses within
  a batch overlap instead of following one another.
  - E-cores, three alternating runs: cycles:u 1.95 to 2.00 → 1.70 to 1.73 G, task-clock
    1129 to 1163 → 987 to 1010 ms, elapsed 0.324 to 0.334 → 0.292 to 0.297 s, instructions:u
    2.66 → 2.93 G. S 412 → 448.
  - P-cores, `perf stat -r 10`, interleaved: task-clock 1073/1067/1075 → 1089/1076/1086 ms,
    elapsed 125/115/128 → 119/121/123 ms. No gain: on a P-core the hyperthread sibling already
    runs while one thread waits on a miss. Reverted.
  - Variants on E-cores: separate intern and OR steps 1.711 to 1.762 G (the fused form was equal or
    slightly faster), explicit `prefetcht0` 1.745 to 1.777 G, and batches of 16 or 64 within noise
    of 32.
- **Prefetching the entry as well as loading the slot early** (local only, P-cores,
  `perf stat -r 10`, interleaved, on top of step 5). In one variant, a prefetch of the entry was
  issued as soon as the slot's value arrived. In the other, a one-record pipeline marked each
  record one record late and prefetched its entry in between. They measured 1044 to 1055 ms and
  1035 to 1053 ms of task-clock, against 1036 to 1068 ms for the early slot load alone (step 7) and
  1145 to 1185 ms for step 5. Neither was kept.
- **Prefetching entries a few letters ahead while marking** (local only, step 9's stage 4,
  medians of 11 interleaved runs): 4, 8 and 16 letters ahead measured 64.17, 63.39 and
  64.46 ms, against 64.18 ms without.
- **A region table indexed by rank** (local only, on stage 5 before the byte-wide table): a
  bitset of the ids present plus a running count, so the group array holds only the 200k
  regions. 65.65 and 67.35 ms against 63.82 and 64.51 ms in two sessions, for 13.99 to 14.45 MiB
  against 15.70 to 15.74 MiB. The byte-wide table with `mallopt` reached 13.9 to 14.1 MiB at no cost
  in time (stage 5).
- **Slots at three-quarters load instead of half** (local only, on the rank-table build):
  69.09 against 65.65 ms, for 11.53 to 12.46 MiB against 13.99 to 14.11 MiB.
- **Pinning each worker** to the i-th CPU it may run on, with `sched_setaffinity` (local only,
  on stage 5 before the byte-wide table): 63.37 against 63.67 ms over 21 runs, and 63.94
  against 64.17 ms over 15. Letting hyperthread siblings mark in alternate phases on top of
  that: 63.44 ms in the second session. Both are within that build's spread across sessions,
  63.4 to 64.5 ms.
- **A longer spin before parking**, 20,000 spins instead of 2,000: 59.71 against 59.44 ms.
- **One name table shared by all parsers, with 4-byte letters** (local, on step 35's code). Each
  parser looked its record's name up in one lock-free table of 262,144 slots, each an id plus one
  and 6 bits of the hash, with a 32-byte key per id. A key was written before a `Release` CAS put
  its id in a slot, and lookups read the slots with `Acquire`. A new name's id was its owner's
  next local index, from one atomic counter per owner, times the thread count plus the owner. A
  letter was then 4 bytes, the local index and the group, and each owner kept a row of 256 bits
  per name it owns, about 270 KB per owner.
  - `ab.py`, 41 interleaved runs, two rounds, medians / minimums: 26.83 / 26.26 → 37.53 / 36.52
    ms and 27.21 / 26.33 → 38.00 / 36.58 ms (+40%). With each name's slot prefetched as soon as
    its hash was known and probed after the region id and the amount, as step 7 did, in a second
    session: 26.60 / 25.81 → 35.92 / 35.43 ms and 26.58 / 25.91 → 35.92 / 35.19 ms (+35%).
    Rounds of 512 KiB, which 4-byte letters allow at one mailbox page each: 35.87 to 36.06 ms.
  - `perf stat`, 15 interleaved runs: instructions 1055.3 → 1078.9 M (+2.2%), cycles 601.7 →
    867.6 M (+44%), L2 misses 2.52 → 7.36 M. Without the marking, the lookup and its letter
    alone ran 408 M cycles more than step 35 with no name work at all, where all of step 35's
    letters and marking cost 228 M. In a profile of one run, a third of the cycles sat on two
    loads: the key's (25% of the samples) and the slot's (11%).
  - The whole table, 1 MB of slots and 3.2 MB of keys, is every core's working set, while a
    P-core's L2 holds 1.25 MB and the two owners on a core take 1.2 MB of step 35's name sets.
    Step 35's 32-byte letters cost bandwidth, but they stream through the mailboxes in order;
    the shared table costs two random loads per record that miss L2. Both run about 1.75
    instructions per cycle, so even with every miss hidden, 2.2% more instructions could not
    have made it faster.
  - memory.peak, 12 alternating runs: medians 12.58 → 13.21 MiB; S 387 → 427, untrimmed. The
    measured build's table did not grow, so it was checked on full seeds 1 to 7 and 10 to 12, which
    give the reference hashes; a build that sizes the table from an upper bound passes the gate
    (310 pass, 0 fail).
- **The region table indexed by rank, again** (local, turbo off, on step 39's code), for memory:
  the 1 MB array of one group byte per region id became a 16-byte entry per 64 ids, the bits of
  the ids in the table and the count of table ids below them, and the groups of the 200,000 rows
  in id order, 0.45 MB at full scale. In step 0 each share's rows set their bits with a locked OR
  and the shares counted their rows, and in step 1 each row's group went to its number among all
  rows. A record's group is the one at its entry's count plus the set bits below its own, so the
  merge waits on a second load that depends on the first, after a `bzhi` and a `popcnt`, and only
  the first load can be prefetched.
  - `ab.py`, 41 interleaved runs, three rounds, medians: 24.92, 24.90 and 24.85 → 26.80, 26.76
    and 26.80 ms (+7.5% to +7.8%). Unchecked, with the ids past the table clamped onto an empty
    last entry: 26.73, 26.67 and 26.77 ms (+7.1% to +7.7%); `perf stat`, 10 interleaved runs,
    instructions:u 936.8 → 995.7 M, cycles:u 570.5 → 630.1 M. With the groups' index masked to
    their first 1 KB, so that the second load always hits L1 (a wrong output, for timing only):
    25.79, 25.84 and 25.82 ms (+3.5% to +3.9%). About half the cost is that load's misses and
    half the longer chain into the merge.
  - The group computed where the region's byte was prefetched, and carried in the batch: 28.91
    against 24.95 ms (+16%). The first load then misses with nothing to overlap it.
  - Building the table moved the first round 0.06 ms later: 1.29 → 1.35 ms from the start of
    `join`, medians of 41 runs.
  - memory.peak, 80 alternating runs: medians 12.28 → 11.76 MiB, highest of five 12.69 → 12.31
    MiB (expected), with the groups' 200 KB mapped in by main, as step 33 does for the byte-wide
    table. Main cannot know the row count before the workers count the rows, so the build
    measured took full seed 1's as given. With each worker mapping in the groups of its own rows
    instead: 12.91 and 13.15 MiB, 0.6 MiB above step 39.
  - S 387 → 420, untrimmed. At 0.345 points per MB and 0.108 per 1% of T, the 0.4 to 0.5 MB
    saved is worth about +0.16 points and +7% of T about -0.8. Even the masked build's +3.5%
    would lose 0.38.
- **The byte-wide table mapped in by the first worker** instead of by main (local, turbo off, on
  step 39's code), so that main would start the threads 0.25 ms sooner, while that worker mapped
  in the 1 MB as the others started: `ab.py`, 41 interleaved runs, medians 24.86 → 25.00 ms;
  memory.peak, 40 alternating runs, medians 12.18 → 12.36 MiB, highest of five 12.61 → 12.96 MiB
  (expected).
- **Main maps in the region table after it starts the workers** (local, turbo off, on step 41's
  code), so that the 0.2 ms of `MADV_POPULATE_WRITE` overlaps the spawns: `parallel` inlined into
  `join` as a `thread::scope` that spawns the workers and then makes the call, with no change in
  lines. Main began spawning 0.20 ms sooner and the first worker started at 0.33 instead of
  0.55 ms, but the last still started at 0.875 ms: each spawn took about 45 µs instead of 27 µs,
  and the first round began at 1.314 against 1.306 ms (instrumented copies, medians of 31 runs).
  `ab.py`, 41 interleaved runs: 23.37 → 23.33 ms. On step 42's code, where `join` already spawns
  the workers in a `thread::scope`, the call moved after the spawns: 23.57 → 23.73 ms.
- **The workers started as a tree**, each thread handing the upper half of its numbers to a new
  thread, as track A's `fan_out` does (local, turbo off, on step 41's code): the last worker
  started at 0.963 against 0.938 ms and the first round at 1.429 against 1.353 ms; `ab.py`, 41
  interleaved runs, 23.37 → 23.93 ms (+2.4%).
- **The workers that finish first spin until the exit** instead of parking (local, turbo off, on
  step 41's code), so that their CPUs are awake when the exit kills their threads: `ab.py`, 41
  interleaved runs, 23.37 → 23.35 ms; the output was written at 21.33 against 21.27 ms.

- **Memory charged ahead per CPU, measured after step 54** (local, not built). The kernel charges
  a cgroup for anonymous and kernel pages in batches of 64 pages per CPU, and memory.peak counts
  the part of each batch not yet used. A test program in which threads, each on a CPU of its
  own, touch pages and then wait at a barrier (memory.peak in a fresh scope, 5 runs, KiB): one
  thread touching 1,536 pages, 7,364 to 7,952; twelve threads touching 128 each, the same 6 MiB,
  9,660 to 10,440; twelve threads touching one page each, 4,516 to 4,808, against 1,992 to 2,704 touching
  none. On step 54, sampling the cgroup's memory.stat during the run (6 runs): peak 10.13 to 10.62
  MiB, of which anonymous 6.80 to 7.08 MiB (name sets about 4.6, mailbox pages 1.1, the region table
  0.95), kernel 0.66 to 0.82 MiB (page tables 0.35 to 0.50), and about 2.4 MiB charged ahead on the
  twelve CPUs. memory.current rises in steps of 0.25 MiB, one batch, and what a CPU holds ahead
  is what is left of its last batch, so it depends on how many pages that CPU charged, modulo 64.
  Variants on step 54's code, seed 1 (peaks over 4 to 8 runs; `ab.py` medians, 31 or 41 runs):
  - Workers on stacks that main maps in (`pthread_create`), whose pages 3, 6 or 12 of them map
    in during the region step, so that the others never fault a page: peaks 8.52, 8.52 and
    9.41 to 9.52 MiB; times +5.2% and +5.5% (seeds 1, 2), +3.4% and +1.6%; S 348 → 422.
  - The same with the standard library's threads: 10.05 to 10.54 MiB, +5.9%.
  - Main making each worker's name set, groups and posting ends, before the spawn: 8.52 to 9.38
    MiB, +3.2% and +3.8%; during the region step: 8.52 to 8.77 MiB, +3.4%. Three workers making
    them: 8.77 to 9.61 MiB, +2.5%. Each worker mapping in its own pages first: 10.31 to 10.52 MiB.
  These peaks repeat run after run because each CPU's page count does, but moving pages from
  one CPU to another moves its leftover either way, and a holdout seed with other counts would
  move it again. Averaged over that, only CPUs that charge nothing save, about 0.125 MiB each,
  against 2.5 to 5.5% of time. Not kept.

## Profile notes

- Knockouts on v1 (E-cores, cycles:u, a noisier run with v1 at 3.08 G): no name table 1.02 G, no
  name compare 2.16 G, no bitset OR 3.19 G (no gain), no region lookup 2.63 G. The name table
  cost about two thirds of all cycles, which is what step 2 went after.
- Knockouts after step 2 (E-cores, cycles:u): full 1.97 G, no name table 1.06 G, no region
  lookup 1.72 G, scan only 1.00 G.
- Phase timeline after step 2 (P-cores, 12 threads, instrumented copy): regions parse 0 to 11.0 ms
  on one thread; quote pre-pass 11.5 to 17.1 ms, about 5.5 ms per thread; workers 17.9 ms to about
  104 ms, 85.1 to 86.8 ms each, so the chunks are balanced; merge about 1 ms; distinct counting
  105.3 to 114.2 ms on one thread; format and write 0.6 ms. About 20 ms of the 115 ms runs on one
  thread.
- Setup timeline at step 39 (P-cores, 12 threads, turbo off, instrumented copy, medians of 21
  runs, from the start of `join`): main's allocations and the region table's 1 MB mapped in take
  0.25 ms; the first worker starts at 0.47 ms and the last at 0.79 ms, in an order that does not
  follow the spawns, which points at idle CPUs waking up; step 0 of the region table ends at
  0.61 to 0.92 ms; the workers that waited longest had gone to sleep and leave that barrier up
  to 0.12 ms after the last arrival; the second barrier passes at 1.22 ms. About 5% of T passes
  before the first round.
- Setup and tail at step 41 (P-cores, 12 threads, turbo off, instrumented copy, medians of 31
  runs, from the start of `main`): `join` starts at 0.09 ms and spawns the workers at 0.34 ms,
  after the region table's 1 MB is mapped in; the workers start at 0.55 to 0.87 ms and end step 0
  of the region table at 0.69 to 1.00 ms; the first round starts at 1.31 ms. The last round ends
  at 20.53 ms, the workers have given back their pages at 20.97 to 21.05 ms, and the last one
  merges in 0.04 ms, formats in 0.15 ms and has written the output at 21.27 ms. `ab.py` measures
  23.4 ms, against 1.6 ms for an empty C program, so about 0.5 ms passes between the write and the
  exit, and before `main`. Whether main spawns early, late or as a tree, the last worker starts
  at 0.87 to 0.96 ms.
