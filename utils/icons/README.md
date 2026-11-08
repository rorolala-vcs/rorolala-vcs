# rorolala-utils-icons

## Introduction

A build script that draws the tree's icons from the descriptions beside them.

## Boundary

It has no runtime code. It owns reading `res/icons/*.toml` and writing the `.ico` files beside the sources, and that is the whole of it. Linking an icon into a program on Windows is that program's build script's work, and using an icon anywhere is the desktop program's.
