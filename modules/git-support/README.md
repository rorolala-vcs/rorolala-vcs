# rola-git-support

The local half of rola's git support: what a project's files say, and what a git work tree is asked.

A project is a directory holding `rola.toml`, the file a person writes. It names the Vaults its
materials come from, what the features do, and one `[[mapping]]` per material. Beside it sits
`rola.lock`, the generated list of paths and the content placed at them; this crate reads it and
never writes it.

This crate holds the parts that are a library rather than a program: reading the project files,
finding the project and the git work tree it is inside, writing the project's block of
`.git/info/exclude`, and moving materials out of the working tree and back. It answers nothing on
its own: the programs are `git-rola`, and the network, the Vault and the store are theirs to reach.

## The private area

What is not part of the work lives under the project's `.rola/git/`:

```text
.rola/
  git/
    staging/      # materials taken out of the working tree, by the path they belong at
    state.toml    # the materials last put into the working tree
```

The data directory is ignored by the project's exclude block, and the whole area can be deleted:
what is missing is put back from a source, and what is stale is corrected on the next command.

## Linked work trees

git keeps one exclude file for the whole repository and reads it in every work tree of it, so two
work trees of one project would write the same managed block from different locks: whichever ran
last would leave the other's materials visible to git. Until a work tree's materials can be kept
apart, a session opened in a linked work tree is refused with [`Error::LinkedWorkTree`], and the
places that consult the repository without acting — the installed hooks, the wrapped git commands —
say so instead of acting there.
