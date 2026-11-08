# rola-changelog

## Introduction

Edits the changelog, recording what changed in `dev/CHANGELOG-DETAILS.md`, generating `CHANGELOG.md` from it, and moving the workspace version, and is reached as `cargo changelog`.

## Boundary

It owns the details, the generated summary, and the version in the root `Cargo.toml`. It does not publish a release or write its body: `.github/workflows/ci.yml` does that from the tagged version's section of the summary. It does not decide whether a change needs an entry, which `.agents/skills/changelog-entry/SKILL.md` is the guide to. Its output is neither translated nor given a table of contents.
