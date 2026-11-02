# Speed Demon 2026: Cause Of Death

Four entries in [Speed Demon 2026](https://speeddemon.dev), one per track, all in Rust with no
crates. Every track's output is byte-identical to the organisers' reference; each folder carries
its own performance log with the board run ID behind every change.

| Track | What it does | Reference | This entry | Memory | Lines | Points |
|---|---|---|---|---|---|---|
| [A: Ingest](track-a/) | log to hourly percentiles | 32.05 s | **16.6 ms** | 16.9 MB | 344 | 87.6 |
| [B: Index](track-b/) | 50k boolean and phrase queries | 56.35 s | **90.6 ms** | 41.8 MB | 901 | 79.4 |
| [C: Join](track-c/) | hostile CSV to country aggregates | 14.44 s | **19.4 ms** | 11.4 MB | 348 | 83.1 |
| [D: Traverse](track-d/) | 2k shortest paths on 5M edges | 72.69 s | **118.4 ms** | 65.7 MB | 424 | 73.6 |

Times and memory are the final code's best board run for each track on the full public dataset,
seed 1, with its run ID in the track's `PERF.md`. Points are the board's score for that run. The
full step-by-step logs are in each folder's `PERF.md`, and the same pages are built from
[`docs/`](docs/) into a docs site: <https://keir.is-a.dev/danse-macabre/>.

## The same five decisions in all four tracks

1. **Map the input, never read it.** Every program `mmap`s `/data` read-only and walks the bytes
   with one cursor, so no line is copied into a string and the page cache is never re-read.
2. **One team of 12 threads, one phase.** Each program spawns a fixed team and splits the input
   into one chunk per thread at safe cut points, then works in phases with short serial steps
   between them.
3. **Compact records instead of repeated work.** A parses 239 MB of log into 12 MB of 4-byte
   records, B indexes only the terms its queries use, C measures escaped fields instead of
   rewriting them, D packs an edge into 4 bytes.
4. **Exact answers, checked by hash.** Percentiles are nearest-rank on a counting sort, sums are
   integers merged in a fixed order, and D's search is Dijkstra with binary weights, not an
   approximation. Nothing depends on thread scheduling.
5. **No teardown.** Each process exits straight after the write, and the threads hand their
   mappings back to the kernel in parallel instead of leaving it to `munmap` at exit.

## Running one track

Each folder builds offline from the pinned images in its `Dockerfile`, with no network:

```
docker build --network=none track-a
```

The board's settings are a read-only `/data`, one writable `/out`, 12 hardware threads and no
swap; `check.py` reproduces them locally against a data directory. For a native build without
Docker:

```
cd track-a && cargo +1.99.1 build --release --offline
```

`TRACK` is `a` for `track-a` and so on, and each program writes one file: `summary.txt`,
`results.txt`, `aggregates.txt` or `distances.txt`.

## What is in the repo

```
track-{a,b,c,d}/   Dockerfile, Cargo project, README, PERF.md, .speed-demon.toml
docs/              mdBook sources; each page includes that track's README and PERF.md
check.py           builds a track with the board's Docker settings and checks the hash
```

## Licence

MIT, the same licence as the organisers' starter. The reference implementations the numbers are
measured against are copyright Hackathon Raptors CIC.
