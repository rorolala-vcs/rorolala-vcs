# rorolala-vcs

## Introduction

The version control model: the `Variant`, `Version`, `Creator` and `Message` objects, the index that keeps them, the chains they form, and the lookback a reader walks.

## Boundary

It owns those four object types and their hashes, the index as a content-addressed store of its own, the chain a version sits at the top of, and the lookback and drawing a command or the Desktop reads. It does not own the Layout, which `modules/layout` keeps, or the reverse index over it, which is `modules/inverse-index`'s. How the work stands is `modules/status`'s question, and the content a variant points at belongs to `modules/storage`.
