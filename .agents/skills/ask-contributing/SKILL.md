---
name: ask-contributing
description: Answer what can be contributed to rorolala and what a contributor's machine needs. Runs `dev/ask-contributing/list.py` to collect the `# Contributing` section of every README, then answers from those sections and from `CONTRIBUTING.md`. Use when the user asks where they can contribute, what they could work on, what to install to build or check the project, or how to start as a contributor.
---

# Ask Contributing

Load this skill when the user asks what can be contributed, where to start, or what a machine needs
to work on this tree. The answer is stated by the tree, in the `# Contributing` section of each
area's README, so read it rather than inventing a list.

## First command

Run the list from the repository root:

```sh
python dev/ask-contributing/list.py
python dev/ask-contributing/list.py app modules
python dev/ask-contributing/list.py --json
python dev/ask-contributing/list.py --strict
```

The script walks every `README.md` under `app/`, `modules/`, `utils/`, `dev/`, `tests/` and `book/`,
and prints the path and the `# Contributing` section of each one that has it. It warns when an area's
own README lacks a section, and `--strict` exits 1 in that case, which is what a check would run.
The README beside the code is the authority; the script only gathers the sections.

## How to answer

1. Run the script before answering anything. Do not list contribution points from memory.
2. Answer in the user's language. Keep paths, commands and identifiers verbatim.
3. Name the area, what can be contributed there and what that area needs, in the words of its own
   section. When the user asks what to install, give the tools the sections name, and say which ones
   every area needs and which ones one area needs.
4. Point at `CONTRIBUTING.md` for the gate: what `cargo ci check` runs, in what order, and what a
   change is expected to carry.
5. When the user asks what to work on, offer the areas whose sections match their interest, and say
   which one is the smallest first change. Do not invent a task.

## What not to do

- Do not claim a requirement the sections do not state.
- Do not run a build or the gate to answer. The sections and `dev/ci/help.txt` are enough.
- Do not edit a README's section while answering unless the user asks for the edit.
