# Linux packaging

## Introduction

Holds the assets a Linux package is assembled from: the user unit, the `.desktop` files, the AppImage's `AppRun`, and the script a package runs.

## Boundary

This directory is the copy the packaging configuration points at. Staging the trees and writing the packager's configuration is `dev/build/src/linux.rs`, and `cargo dev publish` is what runs it. It does not enable or start the daemon: `rolad.service` is a user unit that a package installs and leaves off, and starting it is its user's decision.
