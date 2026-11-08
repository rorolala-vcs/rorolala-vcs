# rola-edit

## Introduction

`rola-edit` is the small terminal editor Rorolala ships as its own fallback, so a run that has to ask for words has an editor even on a machine that names none. It opens one existing file, draws it, takes keys and saves it in place.

## Boundary

It owns the screen, the keys, the cursor and the language of its own messages. It does not choose which editor a run opens: `rola` decides that from `--editor`, `ROLA_EDITOR`, `EDITOR` and this program in that order, and the command that asked owns what the file means and when it is opened. It creates no file: a path that is not there is refused, so the caller writes what it wants edited first.
