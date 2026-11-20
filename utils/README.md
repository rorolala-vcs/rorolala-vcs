# Contributing

`utils/` holds the facilities anything may use: paths, configuration, FFI, CLI drawing, progress,
icons and the test sandbox. A crate here is not meant to depend on another, except the pairs that
are one implementation and its proc macro.

## What can be contributed

- A facility two areas both need. A thing only one crate needs belongs in that crate instead.
- The FFI helpers under `utils/lazyffi`, `utils/lazyffi-core` and `utils/lazyffi-macros`. The C
  header is generated from what they export, so the macro and the header stay one surface.
- The icon build script under `utils/icons`, which reads `res/icons/*.toml` and writes the `.ico`
  beside each picture.
- The test sandbox under `utils/sandbox`, which the integration suites build their directories in.
- The C# facilities under `utils/desktop-i18n`, `utils/desktop-menu` and `utils/desktop-sys-icons`.

## What it needs

- Rust 1.85 or newer and `git`.
- For the C# facilities, the .NET SDK that `global.json` pins.

## Checks

`cargo ci test`, `cargo ci clippy` and `cargo ci doc` cover the Rust side. The C# facilities are
covered by their own test projects under `cargo ci dotnet-check`.
