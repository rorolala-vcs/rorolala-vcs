# rorolala-cli-setups

## Introduction

`rorolala-cli-setups` is the shared start of Rorolala's command line programs: it settles the language, colour, progress, format, pager and concurrency, takes the global `--vault`, `--force` and `--offline` flags out of the run, and hands a command the Vault, Workspace and store it stands in.

## Boundary

It owns the setup values and the resources they register, such as `RorolalaSetup`, `RorolalaDefaultSetup`, `ResVault`, `ResWorkspace`, `ResRorolalaStorage`, `ResForce` and `ResOffline`. It does not own the commands: `gen_program!()` collects them per crate, so a command is declared where it binds. It does not own what a Vault, a Workspace or a store is, which is `librorolala`, nor how a result is drawn, which is `rorolala-utils-cli-theme` and mingling. It also leaves the framework's `-v` and `--version` unmounted, because `-v` is the short name of `--vault` and `-V` is answered by each program's own version node.
