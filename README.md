# Speed Demon 2026: Cause Of Death

These are my four entries for [Speed Demon 2026](https://speeddemon.dev), one per track. I did it
solo, all in Rust with no crates, and every program's output matches the organisers' reference
byte for byte.

| Track | What it does | Reference | Mine | Memory | Lines | Points |
|---|---|---|---|---|---|---|
| [A: Ingest](track-a/) | log to hourly percentiles | 32.05 s | 16.6 ms | 16.9 MB | 344 | 87.6 |
| [B: Index](track-b/) | 50k boolean and phrase queries | 56.35 s | 87.3 ms | 42.1 MB | 961 | 79.6 |
| [C: Join](track-c/) | hostile CSV to country aggregates | 14.44 s | 19.3 ms | 11.3 MB | 348 | 83.1 |
| [D: Traverse](track-d/) | 2k shortest paths on 5M edges | 72.69 s | 118.4 ms | 65.7 MB | 424 | 73.6 |

Each row is the final code's best board run on the full public dataset (seed 1), and the points
are what the board gave that run. Its run ID is in the track's `PERF.md`, along with every step
before it. The same READMEs and logs are built from [`docs/`](docs/) into a small book at
<https://keir.is-a.dev/danse-macabre/>.

## What all four have in common

<!-- ANCHOR: shared -->
1. Every program `mmap`s `/data` read-only and walks the bytes with one cursor, so no line is ever copied into a string and the page cache is never read
   twice.
2. Each one runs a single team of 12 threads. The input is cut into one chunk per thread at safe
   points, and the work goes in phases with short serial steps in between.
3. The data gets packed small: A turns 239 MB of log into 12 MB of 4-byte records, B indexes
   only the terms its queries use, C measures escaped fields without ever rewriting them, and D
   squeezes an edge into 4 bytes.
4. Answers are exact, and the output hash checks that. Percentiles are nearest rank on a counting
   sort, sums are integers merged in a fixed order, and D's search is plain Dijkstra on integer
   weights. Nothing depends on how the threads get scheduled.
5. There's no teardown. Each process exits right after its write, and by then the threads have
   handed their mappings back to the kernel in parallel, so `munmap` at exit has nothing left to
   do.
<!-- ANCHOR_END: shared -->

## Running a track

Each folder builds offline from the pinned images in its `Dockerfile`: the first stage compiles
with `rust:1-slim-trixie` and the second copies the binary onto `debian:13-slim`. No network
needed:

```
docker build --network=none track-a
```

The board runs the image with a read-only `/data`, one writable `/out`, 12 hardware threads and
no swap, and `check.py` reproduces that locally:

```
python3 check.py a --data <folder with the track's input> --expected <sha256>
```

For a native build without Docker, use the same toolchain as the image:

```
cd track-a && cargo +1.99.1 build --release --offline
```

Every track compiles for `target-cpu=alderlake` (in its `.cargo/config.toml`), which is the
scoring box's feature set. I never used `native`, because the board builds on a different host.
Each program writes exactly one file: `summary.txt`, `results.txt`, `aggregates.txt` or
`distances.txt`, for A to D in that order.

## What's in here

```
track-{a,b,c,d}/   Dockerfile, Cargo project, README, PERF.md, .speed-demon.toml
docs/              mdBook sources; each page includes that track's README and PERF.md
check.py           builds a track with the board's Docker settings and checks the hash
```

## Licence

MIT, same as the organisers' starter. The reference implementations my numbers are measured
against are copyright Hackathon Raptors CIC.
