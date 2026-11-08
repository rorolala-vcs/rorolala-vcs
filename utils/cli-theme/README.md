# rorolala-utils-cli-theme

## Introduction

The styles Rorolala's command line programs are drawn in, together with the switch that decides whether anything is drawn with color at all.

## Boundary

It owns the color and theme switches, the `trd!` macro, document and table rendering, and the shapes of an error, a warning and a help line. It does not own what a message says: the wording and the translation files are the command modules' under `app/cli/`, which ask this crate only to draw what they hand it.
