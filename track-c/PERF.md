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

## Experiments that did not stay

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
