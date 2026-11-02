# Speed Demon 2026: Cause Of Death

Four entries, one per track, in zero-crate Rust. Outputs are byte-identical to the reference on
every seed; each track's `PERF.md` names the board run that measured every change.

| Track | Reference | Best run | Memory | Lines | Points |
|---|---|---|---|---|---|
| A: Ingest | 32.05 s | 16.6 ms | 16.9 MB | 344 | 87.6 |
| B: Index | 56.35 s | 90.6 ms | 41.8 MB | 901 | 79.4 |
| C: Join | 14.44 s | 19.4 ms | 11.4 MB | 348 | 83.1 |
| D: Traverse | 72.69 s | 118.4 ms | 65.7 MB | 424 | 73.6 |

## The change to ask about first, per track

**A, keep the file's order.** The log arrives in long runs of one hour, so storing one record per
line and remembering where each run starts removes a sort over 3.1 million lines entirely. That
single layout decision (step 8) freed the memory that made 4-byte records and the 17 MB peak
possible, and the output order then falls out of one sort of the 1,842 endpoint names.

**B, let a sample choose the work.** Tokenising the first 1/64 of the corpus estimates how many
documents hold each term, and each query then gets triggers: terms such that every matching
document contains one of them. Triggers never decide the answer, so a bad estimate costs time
rather than correctness. They are why a phrase is checked only at its rarest term's positions.

**C, shard names by hash instead of locking.** Each name belongs to one thread, and threads trade
16-byte letters in rounds with a barrier between them. The only atomic left in the record path is
one per thread per round, and a name table's line never moves between cores. The reference
contends on every record.

**D, pack the edge and search from both ends.** An edge is one `u32`: neighbour and weight. That
halves both CSRs to 40 MB. Bidirectional Dijkstra then grows two discs of half the radius instead
of one full disc, and unreachable queries end after a single backward step.

## How to read the rest

Each track's page below is that folder's own `README.md` and `PERF.md`, included at build time, so
the site cannot drift from the submission. The performance logs also keep the experiments that
failed, with their numbers: they are the part of the work that is hardest to reconstruct later.
