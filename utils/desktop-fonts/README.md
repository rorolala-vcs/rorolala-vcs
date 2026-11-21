# RorolalaDesktopFonts

## Introduction

Carries the Noto Sans family the Desktop program and the launcher are drawn in, and hands it over as the `FontFamily` both programs and their plugins draw with.

## Boundary

It owns the embedded font files, the address Avalonia reads them from, and the family name. It does not fetch the files, which `cargo dev fonts` does at a pinned commit, and it does not apply the family: the look of each program decides what every word is set in.
