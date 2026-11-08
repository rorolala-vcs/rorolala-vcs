# rorolala-utils-constants

## Introduction

The shared vocabulary of paths, directory names, environment variables and file names that Rorolala's crates agree on.

## Boundary

It owns those names and their values, and no behavior: nothing here reads, writes, creates or checks anything. Locating a path, normalizing it or resolving the data home is `rorolala-utils-location`, and the Workspace and Vault formats are owned by the modules that define them.
