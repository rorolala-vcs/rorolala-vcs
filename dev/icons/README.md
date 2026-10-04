# rorolala-dev-icons

Draws the Desktop's own pictures from the pinned Material Design icon set, and writes what goes beside
them. It is a development tool: it is what `./run.sh icons` runs, and nothing at run time depends on it.

## What it reads

`app/desktop/icons.toml`, which is the one file adding a picture means editing:

```toml
[material]
url = "https://github.com/material-icons/material-icons"
commit = "af0ed9c0e1276bad43c4d6ca8e8aaa283e425195"

[[set]]
plugin = "FileSystemPlugin"
key = "rorolala_file_system"
icons = ["visibility", "visibility_off"]
```

A `set` is one plugin: the pictures are drawn into `app/desktop/Plugins/<plugin>/icons/`, and the keys they
are handed over under are `"<key>.<icon>"`. The set itself is not vendored — it is a shallow clone under
`THIRD-PARTY/`, which git ignores, and the tool puts it at the pinned commit before drawing anything.

## What it writes

- `<plugin>/icons/<icon>.png` — the picture, drawn white on nothing at 96 pixels.
- `<plugin>/icons/LICENSE` — the set's own license, copied from the clone so that it matches the pin.
- `<plugin>/icons/README.md` — where the pictures came from, and what not to do to them.
- `<plugin>/Icons.Generated.cs` — the C# keys and the table a plugin hands the library, so that no one
  writes them twice. The hand-written half of that class is the plugin's own; this half is `partial`.

Everything it writes is committed, so a build needs neither the clone nor a renderer: only drawing them
does, and `export` draws them first so that what ships is drawn from the pin.

## Why the drawing is here rather than in the build

The pictures are shipped resources rather than artifacts local to one machine, and the .NET build has to be
able to stand alone — a `dotnet build` on a fresh checkout must find them. So they are committed, and this is
the step that makes them rather than a step every build takes. `resvg` is a library inside this tool rather
than a program the machine has to have, so what is drawn is the same wherever it is drawn.
