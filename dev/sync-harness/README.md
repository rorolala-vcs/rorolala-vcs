# rorolala-dev-sync-harness

## Introduction

Measures what syncing one store into another costs, phase by phase, between two scratch stores over a real encrypted session.

## Boundary

It owns the two stores it makes, the counting allocator, and the report of each phase. It does not define the sync it measures: the primitive is `sync_storage` in `modules/daemon`, the shapes it crosses with are `modules/protocol`, the store is `rorolala-storage`, and the encrypted session is `rorolala-auth`. It does not score a shape: it prints the numbers for someone to compare.
