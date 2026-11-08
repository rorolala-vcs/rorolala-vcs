# rorolala-utils-location

## Introduction

Locating and normalizing local paths: a root with its relative paths, and the data home a run keeps its own files under.

## Boundary

It owns the `Locate` trait, lexical normalization and the data home lookup. It does not define the Workspace or Vault formats or the directories they keep: those names come from `rorolala-utils-constants`, and the modules that read and write those formats own them.
