# Speed Demon 2026: Cause Of Death

This book is my Speed Demon 2026 entry, one chapter per track. It's all Rust with no crates, and
every output hashes the same as the reference on every seed I tested. Here's where each track
landed, going by the final code's best board run on the public dataset (seed 1):

| Track | Reference | Mine | Memory | Lines | Points |
|---|---|---|---|---|---|
| A: Ingest | 32.05 s | 16.6 ms | 16.9 MB | 344 | 87.6 |
| B: Index | 56.35 s | 87.8 ms | 42.0 MB | 947 | 79.5 |
| C: Join | 14.44 s | 19.3 ms | 11.3 MB | 348 | 83.1 |
| D: Traverse | 72.69 s | 118.4 ms | 65.7 MB | 424 | 73.6 |

## Ideas every track shares

{{#include ../../README.md:shared}}

## How a chapter works

Each chapter is that track's `README.md` followed by its `PERF.md`, pulled straight from the repo
when the book builds, so the site can't drift from the code. The logs keep the experiments that
failed too, with their numbers, because that's the part that's hardest to piece together later.
