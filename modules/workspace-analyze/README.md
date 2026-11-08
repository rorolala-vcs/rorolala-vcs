# rorolala-workspace-analyze

## Introduction

The reading of a Workspace's tree against the Layout it works from: the paths lost, untagged, renamed and modified, with a cache so an unchanged file is not read again.

## Boundary

It owns the walk, the fingerprints that tell a move from a loss and a gain, the tree diff, and the cache the next reading starts from. It only reads: it writes no Layout and no index, and it changes no work. What a change means for a person, and which command settles it, is `modules/status`'s to say. The Layout itself is `modules/layout`'s and the index is `modules/vcs`'s.
