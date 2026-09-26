# RorolalaDesktopSysIcons

The icon a desktop gives a file or a directory, read from that desktop.

## Why

What a folder looks like is the desktop's business. A program that ships its own folder icon looks
wrong on every desktop but the one it was drawn for, and a program that reads the system's looks as
though it belongs wherever it is started.

## What it does per system

- **Linux** — a freedesktop desktop, read through its icon theme: the theme the desktop says it is
  using, then what that theme says it inherits, then `hicolor`, which every theme has to fall back
  to. Themes keep a folder as an SVG more often than as a PNG, and this library cannot draw one, so
  the system's own renderer (`rsvg-convert`) draws it, at the size asked for. The render happens once
  per icon: what comes back is kept.
- **Windows** — the shell, through `SHGetFileInfo`, asked about the kind of thing rather than about a
  path: every directory is drawn with the folder icon, and nothing has to exist on disk to be asked
  about.
- **Anywhere else** — nothing, which a caller draws as it likes. macOS is one of those for now.

## What it is not

- It does not draw anything. What it hands back is an Avalonia `Bitmap` for a caller's `Image`, so
  this library has no opinion about layout, size, or where the picture goes.
- It does not know about a *kind* of file. A file is asked for as a file; an icon per file type would
  take the type, which no system here is asked about yet.

## Using it

```csharp
var icon = SysIcons.Directory(16) ?? Placeholder();

if (icon is not null)
{
    Content = new Image { Source = icon, Width = 16, Height = 16 };
}
```

Answers are cached per kind and per size, including the answers that are nothing: a system with no
icon to give is not worth asking again for every row.
