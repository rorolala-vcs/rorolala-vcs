# daemon-linux

A small GTK 4 window over the machine's `rola-daemon`. It is the Linux twin of
[`daemon-win`](../daemon-win): the same four pages, the same commands behind them, and the same
rule that nothing here is a second implementation of a rule. It links no Rust and depends on no
crate.

```text
app/daemon-linux/
  src/          C sources
scripts/
  daemon-linux-build.sh   builds, and exports under `build/bin`
  daemon-linux-check.sh   the same build with the stricter warnings
  daemon-linux-clean.sh   removes this program's artifacts only
```

`build/bin` is the export directory the Rust programs also land in, and the programs have to share
it because the window looks for `rola-daemon` **beside itself** rather than in a configurable
place. `cargo dev export` therefore builds the GUI after the Rust programs are laid down; a plain
`daemon-linux-build.sh` runs before that and only warns.

## Building

Needs GTK 4.10 or newer with its development files, `pkg-config` and a C11 compiler. The two dialogs
are `GtkAlertDialog` and `GtkFileDialog`, which are 4.10; nothing here uses a deprecated chooser or
message box.

```sh
sh app/daemon-linux/scripts/daemon-linux-build.sh
sh app/daemon-linux/scripts/daemon-linux-check.sh
sh app/daemon-linux/scripts/daemon-linux-clean.sh
```

Intermediates go to `.cache/daemon-linux`, so nothing is left in the source tree. `BUILD_DIR` is the
same override `cargo dev` honours.

## The four tabs

- **Daemon** — starts the `rola-daemon` beside the window, appending its stdout and stderr to the
  log below and showing its exit code. `Stop` sends `SIGTERM` and falls back to `SIGKILL`, because
  `rola-daemon` has no stop subcommand. Closing the window stops a daemon it started and leaves one
  it did not.
- **Registry** — `$XDG_DATA_HOME/rola/daemon/registry.toml` through `list`, `check`, `register` and
  `remove`, with the full path of the selected entry at the bottom. The table is sorted by
  namespace, so there is no reordering. A namespace that is already taken is refused by `register`,
  and the window shows that answer; `Edit` rewrites a line by removing it and registering it again,
  and puts the old one back when the new one is refused.
- **Keys** — `$XDG_DATA_HOME/rola/keys`, listed through `rola key`. Importing is the one action that
  does not go through a command, because the CLI generates keys and does not import them: each
  chosen `.pem` or `.pub` is checked for its extension, its stem and its PEM header, and an existing
  file is overwritten only when the overwrite is confirmed one at a time.
- **Admins** — the `[auth] admins` of the registered Vaults, through `rola vault admin`. The last
  administrator is refused by the CLI and the window shows that answer as it came; the first one is
  written into `vault.toml` by hand, since a Vault with no administrator has nobody who may name one.

Every command is run with `--no-color --no-progress` and `rola` with `--no-pager`, so nothing waits
for input or writes an escape sequence into a log. A failure is shown as the program's own words
rather than translated by the window.

## Where the data lives

The Linux paths are the ones the Rust side reads through `dirs::data_local_dir()`, so the window and
the programs cannot disagree:

```text
$XDG_DATA_HOME/rola/keys/                 private .pem and public .pub keys
$XDG_DATA_HOME/rola/daemon/registry.toml  namespace to Vault path
<vault>/vault.toml                        the `[auth] admins` of a Vault
```

`$XDG_DATA_HOME` falls back to `~/.local/share`, and `ROLA_DATA_HOME` overrides both — the same
variable the integration suites use to keep their files away from the machine's.
