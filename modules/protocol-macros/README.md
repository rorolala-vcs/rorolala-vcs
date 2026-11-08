# rorolala-protocol-macros

## Introduction

The one procedural macro the wire protocol needs: `internal_repeat!`, which emits a template body once per number in a range.

## Boundary

It owns that macro and nothing else. It is an implementation detail of `modules/protocol`, which re-exports it, so a caller depends on `rorolala-protocol` and never names this crate. It does not define an action, an address or any other item the protocol exposes.
