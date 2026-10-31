# Track C: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

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

S 415 → 402, and memory.peak is unchanged at 14.8–15.4 MiB. Because the old value is now used,
the OR compiles to a `lock cmpxchg` loop instead of `lock or`. LLVM can emit `lock bts` for this
pattern, but only if the optimizer has not already rewritten the bit test, which it had here. The
loop did not show in task-clock.

## Step 5: regions parsed in one pass

Profiled alone on a P-core, the old parse of `regions.csv` (200k rows, 2.2 MB) took 10.1–10.5 ms,
all before any other thread starts. It made two passes, and each one split the rows at LF
(1.4–1.6 ms) and then parsed each row (3.5–4 ms), with a search for the comma, a validation loop
per field and a fold per field.

Region ids ascend (spec §2.3), so the last row's id now sizes the array up front and one pass is
enough. That pass stores each region's country code and marks the countries present, and once the
countries are numbered, one walk over the array swaps each code for its group. A row parses as
digits up to the comma, followed by a fixed `,XYZ` and LF that one slice pattern checks, so the
row's end comes out of the parse without a separate search. Alone, the new parse takes 2.9–3.1 ms:
2.5–2.7 ms for the rows, 0.5–0.6 ms for the walk over the 1M-entry array and 0.03 ms to number the
countries.

P-cores, `perf stat -r 10`, three interleaved rounds:

| | step 4 | step 5 |
|---|---|---|
| task-clock (ms) | 1144 / 1149 / 1164 | 1144 / 1150 / 1145 |
| elapsed (ms) | 116.8 / 113.1 / 114.1 | 105.6 / 106.7 / 108.3 |

S 402 → 416, and memory.peak is unchanged at 15.0–15.1 MiB. The parse is now stricter outside
the spec: a blank line, a last row without its LF, or an id larger than the last row's returns an
error, where the old parse accepted them.

## Step 6: quoted fields scanned 32 bytes at a time

`perf record` on P-cores put 76% of the samples in the worker loop, which has the name table
inlined into it, 17% in `text_field`, 4.6% in the quote pre-pass and 0.2% in the regions parse.
Knockouts of step 5 then split the worker loop (P-cores, `perf stat`, each knockout beside its
baseline in the same session):

- Without the name table a run takes 557 ms of task-clock and 58–59 ms elapsed, against
  1129–1142 ms and 106–118 ms. Without the region lookup as well, 553 ms and 56–57 ms. The scan
  is half the time, the name table the other half, and the region lookup about 2%.
- Hashing each name and loading its slot costs 0.1 G cycles: 1.39 G against 1.29 G for the scan
  alone. Finding the arena entry, comparing the name and setting the bit cost 0.92 G more when
  the slot is loaded right after the name. The entry adds 2.7 L2 misses per record and 3.8 M page
  walks, which are active in 7% of all cycles, since the arena has had 4 KB pages since step 3.
- The atomic OR costs 7% of task-clock: 1059–1089 ms when the bitset word is only read, against
  1146–1166 ms with the OR. It also moves lines between cores: 1.37 M loads are served from
  another core's cache, against 0.28 M without it. Testing the bit before the OR (1172–1199 ms)
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

S 416 → 426, and memory.peak is unchanged, 14.7–15.0 MiB. On the board this code measured
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
the harness's flags is unchanged, 14.8–15.0 MiB. On the board this code measured 79.9 ms in
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
with the harness's flags is unchanged, 14.7–14.8 MiB against 14.8–15.2 MiB for step 7.

## Step 9: each distinct name owned by one thread

Steps 2 to 7 tuned one name table that all threads shared, and it still cost about half the
time. With all name hashing, posting and marking knocked out of the prototype below, the rest ran
in 42.6 ms (local, median of 11), against 85–88 ms for step 7 in the sessions around it. In
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
  peaked at 19.8–20.2 MiB. Freeing the regions parse's 2 MB scratch array had raised glibc's
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
spends 31.5–32.1 ms parsing and posting, 4.5–5.5 ms waiting at 257 barriers (parking 18–21
times), and 10.2–11.0 ms marking. A wait takes 4–6 µs at the median, 13–21 µs at p90 and
330–380 µs at p99. Of the marking, reading the letters takes 2.44 ms per worker and finding
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
With the private tables, THP no longer changes the peak: 13.80–14.18 MiB with the `prctl` and
13.76–14.28 MiB without it (4 runs each), so the call stays only as insurance. S 447 → 587.

Correctness checks: the gate (307 pass, 0 fail); fresh seeds 5–7; full seeds 2 and 3 at 1 and
64 threads; and the 400 random cases plus two long-name sets against the reference, at 1, 3,
12, 37 and 64 threads, and again with 100-byte rounds and 3-letter mailboxes at 1, 2, 3, 12
and 64 threads, which forces thousands of rounds and early round ends. The long-name sets hold
every name length from 0 to 80 bytes, names equal in their first 24 bytes, trailing-space
twins, `""` escapes at the 8-byte word edges, and quoted and unquoted spellings of one value.

## Step 10: guessed run starts, and the input released by the workers

This step goes after the time outside the worker loop. A timeline of step 8 (P-cores, an
instrumented copy) put the regions parse at 3.1–3.3 ms, the quote pre-pass at 6.4–6.7 ms, the
workers at 62–68 ms, the merge at 0.7–1.0 ms and the write at 0.05 ms. When the process
exits, the kernel unmaps the 200 MB input on one thread: an explicit `munmap` took 2.8 ms, and
the time from the program's last timestamp to the end of the wall was 4.5–5.1 ms (medians of
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

The release saves 1.1–1.5 ms and the guesses 4.4–5.0 ms; together 5.5–6.3 ms. In the quietest
`perf stat -r 10` round, task-clock was 501.4 ms for step 9, 511.3 with the release, 490.4 with
the guesses and 501.6 with both. Page faults stay at 6,675: the workers now fault the input in
as they parse it.

memory.peak in the container with the harness's flags and 12 CPUs, 12 alternating runs each:
13.3–14.3 MiB (mean 13.99) for step 9 and 13.9–14.6 MiB (mean 14.16) for step 10. S 587 → 630:
10 lines for the release and 33 for the guesses, their check and the tail.

Correctness checks:

- The gate (307 pass, 0 fail). Every guess held on all 300 tiny seeds at 1, 3, 12, 37 and 64
  threads, and on full seeds 1–7 at 1, 12 and 64 threads.
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
(13.35–14.01) for step 10 and 12.39 MiB (11.78–12.73) for step 11. S 630 → 622.

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

memory.peak in the container with the harness's flags, 6 runs each: 12.7–13.3 MB for step 11
and 12.5–13.5 MB for step 12. Page faults stay at 6,390. S 622 → 563: the pre-pass −23 lines,
the tail −27 (−40 there, +13 for the mapping and its constants), `Group::EMPTY` −4, the output
walk −5.

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
code were put back: each cost 0.15–0.3 ms. `ab.py`, 50 interleaved runs, median / min: step 12
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
1.637 → 1.549 G (−5.4%), cycles and task-clock flat, page faults 6,390 in both. Merged with
step 14, S 534 → 523, and 522 after inlining the single-use `KEY` constant, which leaves the machine code
identical. Gate: 307 pass, 0 fail; full seeds 1–7; the 400 random and 400 hostile
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
0.953–0.958 → 0.919–0.920 G (−3.7%), task-clock 425–427 → 411–412 ms, instructions 1.550 →
1.655 G, page faults 6,390 in both. In 101-run sessions, batches of 3 and 4 measured −2.8%, 2
−1.2%, 6 −1.8% and 16 −1.9%, and the batch of 4 without the prefetch +1.3%. S 522 → 534.

Variants that did not stay, against step 15 in the same sessions:
- The prefetch alone, with the lookup where it was: flat (41.05 against 41.07 ms).
- Each record's lookup one record late instead of in batches: −0.8% to −1.1%. The record carried
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
  session, 12.17–12.94 → 12.27–13.86 MB.
- Correctness: the gate, 307 pass, 0 fail; full seeds 1–7 give the reference hashes; the 402
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
gate, 307 pass, 0 fail; full seeds 1–7 give the reference hashes; the 402 fuzz cases pass at the
default thread count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12
and 64; the 300 tiny seeds pass at 10 thread counts with 100-byte rounds; clippy reports no
warnings.

## Step 24: 1 fewer line

`main` maps both input files through one closure that takes the file's name. Only `main`'s code
changes, with `map_file` now inlined into the closure, and `NameSet::read` moves 96 bytes down
the binary, onto a 32-byte boundary instead of a 64-byte one. S 295 → 294. `ab.py` against
step 23, interleaved, medians / minimums: 101 runs on seed 1, 39.30 / 38.16 → 39.31 / 38.34 ms;
101 runs on seed 2, 39.24 / 37.95 → 39.21 / 38.06 ms; 61 runs on seed 3, 39.08 / 38.17 →
39.09 / 38.15 ms. Correctness: the gate, 307 pass, 0 fail; full seeds 1–7 give the reference
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
0 fail; full seeds 1–7 and 10–12 give the reference hashes; the 402 fuzz cases pass at the
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
means 12.49 → 12.69 MB, inside this machine's spread of 12.1–13.6 MB for both. Correctness: the
gate, 310 pass, 0 fail; full seeds 1–7 and 10–12 give the reference hashes; the 402 fuzz cases,
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
machine's spread of 11.7–14.0 MB, and no allocation changed. Correctness: the gate, 310 pass,
0 fail; full seeds 1–7 and 10–12 give the reference hashes; the 402 fuzz cases pass at the
default thread count, at 1, 3, 12, 37 and 64 threads, and with 100-byte rounds at 1, 2, 3, 12
and 64; the 300 tiny seeds pass at 10 thread counts with 100-byte rounds; the region sets give
step 27's results; clippy reports no warnings.

## Experiments that did not stay

- **PGO** (local, turbo off, on step 27's code): a profile from three runs on each of full seeds
  2–4, passed as `-C profile-use`, with every function matching it. `ab.py`, 40 interleaved runs,
  medians / minimums: 36.35 / 35.39 → 37.71 / 36.69 ms and 36.27 / 35.64 → 37.39 / 36.79 ms on
  seed 1; 37.38 / 36.22 → 38.55 / 37.47 ms and 37.14 / 36.35 → 38.39 / 37.31 ms on seed 10, which
  it was not trained on. `perf stat -r 5`: instructions:u the same, 1.535 G; cycles:u 0.841 → 0.875 G.
- **The empty name counted by its posters** (local, on step 25's code). Every record with an
  empty name posts its letter to one owner, which then gets 11% more letters than the others
  (306k against 273–277k at full scale) and marks for 0.4–0.5 ms longer. Each poster instead kept
  one bit per country for the empty name, ORed together at the end. Not posting those letters
  measured the same, 37.33 against 37.16 ms (`ab.py`, 40 interleaved runs, medians), and keeping
  the bits in the batch loop, with or without a branch, cost 2.2–2.3 ms through the loop's
  register allocation.

- **The name slots written at allocation** (`[0].repeat(n)` instead of `vec![0; n]`, local, turbo
  off, on step 17), after track B found that a first load from a calloc'd page maps the shared
  zero page and the store after it interrupts every core: `ab.py`, 40 interleaved runs, medians
  40.16 → 40.18 ms on seed 1 and 39.74 → 39.80 ms on seed 2; page faults 6,350 → 6,351, kernel
  cycles 64.8 → 62.9–64.2 M. The slots do not fault that way here.
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
- **Prefetching the entry as well as loading the slot early** (local only, P-cores,
  `perf stat -r 10`, interleaved, on top of step 5). In one variant, a prefetch of the entry was
  issued as soon as the slot's value arrived. In the other, a one-record pipeline marked each
  record one record late and prefetched its entry in between. They measured 1044–1055 ms and
  1035–1053 ms of task-clock, against 1036–1068 ms for the early slot load alone (step 7) and
  1145–1185 ms for step 5. Neither was kept.
- **Prefetching entries a few letters ahead while marking** (local only, step 9's stage 4,
  medians of 11 interleaved runs): 4, 8 and 16 letters ahead measured 64.17, 63.39 and
  64.46 ms, against 64.18 ms without.
- **A region table indexed by rank** (local only, on stage 5 before the byte-wide table): a
  bitset of the ids present plus a running count, so the group array holds only the 200k
  regions. 65.65 and 67.35 ms against 63.82 and 64.51 ms in two sessions, for 13.99–14.45 MiB
  against 15.70–15.74 MiB. The byte-wide table with `mallopt` reached 13.9–14.1 MiB at no cost
  in time (stage 5).
- **Slots at three-quarters load instead of half** (local only, on the rank-table build):
  69.09 against 65.65 ms, for 11.53–12.46 MiB against 13.99–14.11 MiB.
- **Pinning each worker** to the i-th CPU it may run on, with `sched_setaffinity` (local only,
  on stage 5 before the byte-wide table): 63.37 against 63.67 ms over 21 runs, and 63.94
  against 64.17 ms over 15. Letting hyperthread siblings mark in alternate phases on top of
  that: 63.44 ms in the second session. Both are within that build's spread across sessions,
  63.4–64.5 ms.
- **A longer spin before parking**, 20,000 spins instead of 2,000: 59.71 against 59.44 ms.

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
