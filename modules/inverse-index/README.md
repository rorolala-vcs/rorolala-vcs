# rorolala-inverse-index

## Introduction

The reverse dependency index over the VCS index: which objects point at a stored object, and by which role.

## Boundary

It owns the derived table, the covered-key check that decides whether it may be trusted, incremental and full rebuilds, the version numbers it fills as chains become known, and the snapshot and graph windows a reader walks. It is derived data and never the truth: the objects are `modules/vcs`'s, and anything the table says can be rebuilt from them. It does not read or fetch a missing object: a key the index does not hold is reported as missing, and bringing it is the caller's, through the fetch layer in the root `src/`. The index file layout is `modules/vcs`'s.
