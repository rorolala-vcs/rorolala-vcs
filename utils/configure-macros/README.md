# rorolala-utils-configure-macros

## Introduction

The `Configure` derive, which marks a type as the contents of a configuration file.

## Boundary

It owns only the derive expansion and has no runtime code. The trait, the staged file and the formats belong to `rorolala-utils-configure`, which re-exports this crate, so a caller depends on that crate and never on this one.
