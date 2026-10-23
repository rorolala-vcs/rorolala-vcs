# daemon-win

A small Win32 window over the machine's `rola-daemon`. The window is a shell: every action it
offers is one the command line programs already answer, so nothing here is a second implementation
of a rule. It links no Rust and depends on no crate.

```text
app/daemon-win/
  src/          C sources
  res/          the dialog script and the manifest
scripts/
  daemon-win-build.ps1   builds, and exports under `build/bin`
  daemon-win-check.ps1   the same build with `/analyze`
  daemon-win-clean.ps1   removes this program's artifacts only
```

`build/bin` is the export directory the Rust programs also land in, and the programs have to share
it because the window looks for `rola-daemon.exe` **beside itself** rather than in a configurable
place. An export therefore builds the GUI after the Rust programs are laid down; a plain
`daemon-win-build` runs before that and only warns.

## Building

Needs Windows 10/11 and the C++ x64 tools of a Visual Studio Build Tools installation; the script
finds them through `vswhere` and `vcvars64.bat` and refuses rather than guessing. Intermediates go
to `.cache/daemon-win`, so nothing is left in the source tree.

```powershell
.\app\daemon-win\scripts\daemon-win-build.ps1
.\app\daemon-win\scripts\daemon-win-check.ps1
.\app\daemon-win\scripts\daemon-win-clean.ps1
```

## The four tabs

- **Daemon** 鈥?starts the `rola-daemon.exe` beside the window, appending its stdout and stderr to
  the log below and showing its exit code. `Stop` is `TerminateProcess`, because `rola-daemon` has
  no stop subcommand. Closing the window stops a daemon it started and leaves one it did not.
- **Registry** 鈥?`%LOCALAPPDATA%\rola\daemon\registry.toml` through `list`, `check`, `register`
  and `remove`, with the full path of the selected entry at the bottom. The table is sorted by
  namespace, so there is no reordering. A namespace that is already taken is refused by
  `register`, and the window shows that answer; `Edit` rewrites a line by removing it and
  registering it again, and puts the old one back when the new one is refused.
- **Keys** 鈥?`%LOCALAPPDATA%\rola\keys`, listed through `rola key`. Importing is the one action
  that does not go through a command, because the CLI generates keys and does not import them:
  each chosen `.pem` or `.pub` is checked for its extension, its stem and its PEM header, and an
  existing file is overwritten only when the overwrite is confirmed one at a time.
- **Admins** 鈥?the `[auth] admins` of the registered Vaults, through `rola vault admin`. The last
  administrator is refused by the CLI and the window shows that answer as it came; the first one is
  written into `vault.toml` by hand, since a Vault with no administrator has nobody who may name
  one.

Every command is run with `--no-color --no-progress` and `rola` with `--no-pager`, so nothing waits
for input or writes an escape sequence into a log. A failure is shown as the program's own words
rather than translated by the window.

## Where the data lives

```text
%LOCALAPPDATA%\rola\keys\                 private .pem and public .pub keys
%LOCALAPPDATA%\rola\daemon\registry.toml  namespace to Vault path
<vault>\vault.toml                        the `[auth] admins` of a Vault
```

