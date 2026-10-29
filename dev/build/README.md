# rola-build

What this workspace's artifacts are made of, and how they are laid out.

This is the product side of the build: the library and the programs, the icons a package carries,
the export a hand-off is made of, and the package itself. It is reached as `cargo dev <command>`,
through the alias in `.cargo/config.toml`; what _checks_ the tree is `rola-ci`, reached as
`cargo ci <command>`.

The crate is a workspace of its own, and deliberately not a member of the workspace whose artifacts
it builds: cargo unifies a dependency's features across one workspace build, so a dev crate inside
the main workspace would be built with the programs' `pathf` and `structural_renderer` — features
that make `gen_program!()` demand path analysis and serde from its own crate, neither of which a
build tool has any use for. The same arrangement is what mingling reaches for its own `dev/ci`, and
for the same reason.

## What is where

- `src/paths.rs` — where a build works: the workspace, its directories, the programs, the projects,
  and every override checked before it names anything an `export` or a `clean` removes.
- `src/platform.rs` — how a platform spells an artifact; written for every platform rather than for
  the one it is compiled on, so a spelling can be checked by a test on any machine.
- `src/toolchain.rs` — the tools a step runs (`CARGO`, `DOTNET`, `RSVG`, `POWERSHELL`, `SH`, `GIT`,
  `TAR`, split the way a shell splits them) and how a run is made, captured, or given a working
  directory and environment.
- `src/dotnet.rs` — the .NET side: the offline-first restore, the solution, the two programs a layout
  holds, the plugins, the bindings, and the suites.
- `src/iscc.rs` — the compiler a Windows installer is built with, and what to do when it is missing.
- `src/nfpm.rs` — the packager the Linux deb, rpm and Arch packages are built with; pinned like the
  installer's compiler.
- `src/appimagetool.rs` — the tool the Linux AppImage is made with; pinned like the packager, by
  release and hash, since only the versioned release has bytes that hold still.
- `src/linux.rs` — what Linux is handed: the three split packages, the `rorolala-full` meta
  package, the all-in-one `rorolala` package, and the AppImage.
- `src/fs.rs` — copying and removing, naming the file in a failure.
- `src/ops/` — one module per command: `lib`, `bin`, `build`, `icons`, `chunker`, `cargo-clean`,
  `desktop`, `export`, `publish`, `clean`.
- `../packaging/linux/` — the `.desktop` files, the user unit, the AppImage's `AppRun`, and the
  scripts a package runs; the templates say `@COMMENT@` where the workspace's description goes.
