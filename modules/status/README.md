# rorolala-status

## Introduction

How the work stands: the tree beside the Layout it works from, who holds what, the merges in progress, the chain of one file, and whether the content a variant, a version or a Layout entry names is stored here.

## Boundary

It owns the reading that `rola status` and the Desktop Overview present, the ownership comparison against the fetched Vault Layout, the variant files found away from their merge record, the relation of a local version to the upstream one, the per-file chain and variant graph the ABI carries, and the local half of the existence reading, which content a target names and whether every object it needs is held. It does not compute the tree diff, which `modules/workspace-analyze` does, or walk the index, which `modules/vcs`'s `read_chain` does, and it does not ask a Vault which one holds missing content, which `librorolala` does over the exchange. It never changes anything: settling a lost path or a changed one belongs to commands such as `rola align` and `rola track`, and the words a person reads belong to the program that shows them.
