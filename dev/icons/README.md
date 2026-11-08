# rorolala-dev-icons

## Introduction

Draws the Desktop's pictures from the Material Design set at the commit `app/desktop/icons.toml` pins, and writes the license, the C# keys and a preview page beside them.

## Boundary

It owns the drawing, the manifest read, and the generated `Icons.Generated.cs` half of each plugin's keys. It does not own the artwork, which is the upstream set at the pinned commit, cloned under `THIRD-PARTY/` and ignored by git, nor the hand-written half of `Icons.Generated.cs`, which is the plugin's, nor what the Desktop does with the pictures at run time. Nothing at run time depends on this tool: what it writes into the tree is committed, so a build needs neither the clone nor a renderer.
