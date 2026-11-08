# rorolala-dev-chunker-harness

## Introduction

Scores each chunking policy against frozen general-purpose yardsticks over a corpus of version chains, and writes the score beside the corpus.

## Boundary

It owns the measurement: the yardsticks, the predicted cost of a chain, the comparison with a real store, and the `SCORE.md` it generates. The policies it measures are `modules/chunking`, and the store whose bytes it predicts is `rorolala-storage`; a chunker whose ranges are wrong gets no score at all. It does not carry the corpus bytes: a corpus is local to its author, and only `policy.toml` and `SCORE.md` are committed.
