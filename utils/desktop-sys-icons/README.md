# RorolalaDesktopSysIcons

## Introduction

Asks the desktop for the icon it gives a file or a folder, at the size asked for.

## Boundary

It owns the per-system lookup on a freedesktop desktop and on Windows, and the cache of answers per kind and per size. It does not draw, lay out or sniff content: what it hands back is an Avalonia `Bitmap` for a caller's `Image`, the layout belongs to `app/desktop`, and a file's kind is what its name says.
