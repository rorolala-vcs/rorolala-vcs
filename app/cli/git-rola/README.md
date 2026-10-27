# git-rola

Git support for rola, as a program Git reaches for as `git rola`: Git runs `<name>-<word>` for a
word it does not know, so the name is where the two meet.

It follows git's manners rather than rola's banners: a command is quiet when it succeeds, every
failure is one line on stderr beginning `git-rola: `, and `--` separates options from file names.
Where a command lists what changed it uses git's short-status shape, ` M <path>` and ` D <path>`.
The help is git's too: `git rola --help` is a usage line and the commands, `git rola <command>
--help` is that command's usage, and both are written to stdout and end successfully.

What it says is translated the way the rest of rola says it: every word comes from `i18n/` beside
the program, in the language `--lang` names or the first of `ROLA_LANG`, `APP_LANG` and `LANG` that
is set. The language covers the help and the refusals as much as the results.

The work is done through `rola-git-support`: the project is a directory holding `rola.toml`, the
materials it places are listed in `rola.lock`, and what is not in the working tree is kept under
the project's `.rola/git/`. `update` is the one command that reaches a Vault: it resolves each
mapping's pin to the version the Vault's Layout names (or a number in its chain), reads the content
that version points at, brings it into the project's store — `.rola/storage/`, the store every
other command reads from — and writes the lock. Nothing is written unless every mapping resolves.

A repository's exclude file is the one file it keeps for all of its work trees, so this program acts
in the repository's main work tree only. A linked work tree is refused by every command, by the
wrapped git commands before git is reached, and is said by the hooks rather than failed through
them: one work tree's lock must not be able to make another's materials visible to git.
