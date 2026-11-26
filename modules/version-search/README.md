# rorolala-version-search

## Introduction

Naming a version of a chain by a whole hash, the head of one, a variant's hash, a step count, or text of its creator or message.

## Boundary

It owns the levels a chain is named through and the search over them. It does not read the index, which `modules/vcs` does, or own a `Version`, a `Variant`, a `Creator` or a `Message`, which `modules/vcs` keeps, and it does not draw a chain, which `modules/vcs`'s lookback does.
