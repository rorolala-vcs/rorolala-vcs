# rorolala-ignore

## Introduction

The ignore rule set a Workspace or a Vault is matched by, read from the rule files under its `info/ignore/` directory.

## Boundary

It owns the gitignore syntax, the ordering across rule files by depth and then by name, the default file written into a new Vault, and the C ABI over a built set. It does not decide which directory is a Workspace's or a Vault's: locating either is `modules/workspace` and `modules/vault`. It does not walk a tree, which `modules/tree` does when it applies these rules to a tree's paths.
