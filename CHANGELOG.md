# CHANGELOG

This document records all Changelog entries for Rorolala VCS starting from 0.1.0, managed by the `cargo changelog` command built into this project.

# 0.1.0 (2026-10-26)

## Feature

### 1. [`new`] Add the first usable release of the Rorolala VCS suite

This change adds the Rorolala VCS 0.1.0 suite, as follows:

- Added the complete VCS subcommands to the `rola` CLI, including:
  - VCS subcommands track, retrack, checkin, fetch, sync, pack, status, entries, align, etc. (`app/cli/rola/src/`)
  - Commands related to `vcs-index`, `storage`, `layout`, and `ownership` (`app/cli/rola/src/vcs_index/`, `app/cli/rola/src/storage/`, `app/cli/rola/src/layout/`, `app/cli/rola/src/cmd_ownership.rs`)
- Added new Rust modules, including:
  - `modules/vcs` (versions, variants, creators, messages, index, and lookback) (`modules/vcs/`)
  - `modules/status` (status, version graph, merging, and ownership) (`modules/status/`)
  - `modules/tree-analyze` (scanning, fingerprints, tree diffs, and caching) (`modules/tree-analyze/`)
  - `modules/chunking` (chunking strategy registry, including the `blend-basic` and `psd-basic` corpora) (`modules/chunking/`)
- Added VCS-related remote actions to the daemon, including:
  - Creating/writing remote entries, fetching layouts, requesting/abandoning ownership, syncing storage and indexes, etc. (`modules/daemon/src/actions/`)
- Added the desktop-side `RorolalaVCSPlugin`, including:
  - Commands such as checkin/fetch/sync/track (`app/desktop/Plugins/RorolalaVCSPlugin/CheckinCommand.cs`, `Fetch.cs`, `Sync.cs`, `TrackCommand.cs`, etc.)
  - Account and ownership operations (`app/desktop/Plugins/RorolalaVCSPlugin/AccountActions.cs`, `OwnershipActions.cs`)
  - Version graph view, progress panel, and report view (`app/desktop/Plugins/RorolalaVCSPlugin/VersionGraphView.cs`, `ProgressPanel.cs`, `ReportView.cs`)
  - Sync watching and status badge (`app/desktop/Plugins/RorolalaVCSPlugin/SyncWatcher.cs`, `RorolalaStandingBadge.cs`)
- Added accompanying test projects and i18n resources, including:
  - Test cases for checkin/fetch/retrack/variants in the test projects `tests/vcs-index`, `tests/vcs-index-remote`, `tests/vcs-index-sync`, `tests/sync-chain`, and `tests/desktop-host` (`tests/`)
  - i18n resources `cmd_vcs_index*.yml` and `rorolala_vcs/strings.yml` (`app/cli/rola/i18n/`, `app/desktop/Plugins/RorolalaVCSPlugin/i18n/rorolala_vcs/strings.yml`)
