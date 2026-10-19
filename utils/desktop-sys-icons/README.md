# RorolalaDesktopSysIcons

The icon a desktop gives a file or a directory, read from that desktop.

## Why

What a folder looks like is the desktop's business. A program that ships its own folder icon looks
wrong on every desktop but the one it was drawn for, and a program that reads the system's looks as
though it belongs wherever it is started.

## What it does per system

- **Linux** — a freedesktop desktop, read through its icon theme: the theme the desktop says it is
  using, then what that theme says it inherits, then `hicolor`, which every theme has to fall back to.
  A file's kind comes from the system's own MIME glob table (`globs2`), so a PSD is asked for as
  `application-vnd.adobe.photoshop` and a PDF as `application-pdf`; a type the theme has no picture for
  falls back to the generic picture of its top-level kind — an image, a sound, a document — rather than
  to the text icon every file used to get. Themes keep an icon as an SVG more often than as a PNG, and
  this library cannot draw one, so the system's own renderer (`rsvg-convert`) draws it, at the size
  asked for. The render happens once per icon: what comes back is kept.
- **Windows** — the shell, through `SHGetFileInfo`, asked about the file's own name and an attribute
  rather than about a path: the extension is what the shell reads the kind from, so a PSD gets the
  PSD icon and a folder gets the folder icon, and nothing has to exist on disk to be asked about. The
  shell keeps an icon at four sizes — 16, 32, 48 and 256 pixels — and the one wanted is read from the
  list that holds it, so what a caller draws at the size it asks for is the size the shell has and not
  one size stretched: only 56 to 256, which the shell keeps nothing in between, is read at 256 and
  drawn smaller.
- **Anywhere else** — nothing, which a caller draws as it likes. macOS is one of those for now.

## What it is not

- It does not draw anything. What it hands back is an Avalonia `Bitmap` for a caller's `Image`, so
  this library has no opinion about layout, size, or where the picture goes.
- It does not sniff content. A file's kind is what its name says — the extension on Windows and in
  the MIME glob table, the whole name for a `Makefile` — and a file that lies about its name is drawn
  as the name says.

## Using it

```csharp
var folder = SysIcons.Directory(16) ?? Placeholder();
var document = SysIcons.File("report.pdf", 16) ?? Placeholder();

if (document is not null)
{
    Content = new Image { Source = document, Width = 16, Height = 16 };
}
```

Answers are cached per kind and per size, including the answers that are nothing: a system with no
icon to give is not worth asking again for every row.
