# Rorolala Desktop — Specification

**Status:** normative. This document is the specification for the Desktop program. The document
is version-controlled.

**Scope:** the Desktop program at `app/desktop` (Avalonia, `net8.0`). It does not specify the
`rola` command line, the C ABI, or the storage engine, except where this program depends on them.

**Change rule:** every decision below is deliberate. A change to a decision is a change to this
document first, then to the code.

## Table of Contents

- [1. Purpose and Scope](#1-purpose-and-scope)
- [2. Terminology](#2-terminology)
- [3. Architecture](#3-architecture)
  - [3.1 Host kernel](#31-host-kernel)
  - [3.2 Contract assembly](#32-contract-assembly)
  - [3.3 Plugins](#33-plugins)
  - [3.4 Shared and private assemblies](#34-shared-and-private-assemblies)
- [4. Plugin Model](#4-plugin-model)
  - [4.1 Discovery](#41-discovery)
  - [4.2 Identity and manifest](#42-identity-and-manifest)
  - [4.3 Assembly loading](#43-assembly-loading)
  - [4.4 Contract and Avalonia version check](#44-contract-and-avalonia-version-check)
  - [4.5 Dependencies and load order](#45-dependencies-and-load-order)
  - [4.6 Lifecycle and reload](#46-lifecycle-and-reload)
- [5. Configuration](#5-configuration)
  - [5.1 Location](#51-location)
  - [5.2 plugins.json](#52-pluginsjson)
  - [5.3 preference.json](#53-preferencejson)
  - [5.4 theme.json](#54-themejson)
  - [5.5 Validation and failure](#55-validation-and-failure)
  - [5.6 Exit codes](#56-exit-codes)
  - [5.7 Startup sequence](#57-startup-sequence)
- [6. Extension Points](#6-extension-points)
  - [6.1 Overview](#61-overview)
  - [6.2 Top menu](#62-top-menu)
  - [6.3 Context menus](#63-context-menus)
  - [6.4 Navigation buttons](#64-navigation-buttons)
  - [6.5 Docks](#65-docks)
  - [6.6 Open hooks](#66-open-hooks)
  - [6.7 Icon badges](#67-icon-badges)
  - [6.8 Languages](#68-languages)
- [7. Dock System](#7-dock-system)
  - [7.1 Registration](#71-registration)
  - [7.2 Open modes and placement](#72-open-modes-and-placement)
  - [7.3 Instances and layout persistence](#73-instances-and-layout-persistence)
  - [7.4 Core docks](#74-core-docks)
  - [7.5 Bundled plugin docks](#75-bundled-plugin-docks)
  - [7.6 Headers and the strip](#76-headers-and-the-strip)
  - [7.7 Browser interaction](#77-browser-interaction)
  - [7.8 The file agent](#78-the-file-agent)
- [8. Open Hook Pipeline](#8-open-hook-pipeline)
  - [8.1 Request state](#81-request-state)
  - [8.2 Stages and order](#82-stages-and-order)
  - [8.3 Rejection](#83-rejection)
  - [8.4 Exceptions](#84-exceptions)
  - [8.5 After-open](#85-after-open)
- [9. Icon Badges](#9-icon-badges)
- [10. Theming](#10-theming)
- [11. Internationalization](#11-internationalization)
- [12. Logging](#12-logging)
- [13. Rorolala Capability Injection](#13-rorolala-capability-injection)
- [14. Failure Model](#14-failure-model)
  - [14.1 Fatal failures](#141-fatal-failures)
  - [14.2 Non-fatal failures](#142-non-fatal-failures)
  - [14.3 Popup and log deduplication](#143-popup-and-log-deduplication)
- [15. Configuration Reference](#15-configuration-reference)
- [16. Contract Reference](#16-contract-reference)
- [17. Feel and Interaction Craft](#17-feel-and-interaction-craft)
- [18. Non-Goals](#18-non-goals)
- [19. Open Items](#19-open-items)
- [20. References](#20-references)

## 1. Purpose and Scope

Desktop is a **pure file browser**. It carries no version-control semantics of its own. It provides
a window, navigation, docking, menus, and the basic interaction of opening a file or a directory.
Everything that is specific to Rorolala — recognising a Workspace or a Vault, opening an asset with
its history, syncing, comparing — is contributed by **plugins**.

The program is a host for plugins. The host is the shell; plugins are the substance; `rola` is the
source of capability.

This specification covers:

- how plugins are discovered, loaded, ordered, and identified;
- the configuration files that drive the program and the plugins;
- every extension point the host exposes;
- the behaviour of the program when something fails.

## 2. Terminology

| Term | Meaning |
| --- | --- |
| **Host** | The Desktop program itself: window, dock host, menu bar, plugin loader, logging, theming, i18n. |
| **Kernel** | The part of the host that is always present and cannot be disabled: the plugin manager and the log. |
| **Contract** | The shared assembly `RorolalaDesktop.Contract` that both host and plugins compile against. |
| **Plugin** | A .NET assembly that implements `IRolaPlugin` and is loaded by the host. |
| **PluginId** | The stable identity of a plugin. A dotted snake_case string, e.g. `rorolala.file_system`. |
| **Dock** | A panel hosted in the dock area. Identified by a globally unique `DockNameId`. |
| **Entry** | A filesystem item shown by the browser: a file or a directory. |
| **nameid** | A stable, human-readable identifier (used for plugins and docks) as opposed to a runtime numeric handle. |

## 3. Architecture

### 3.1 Host kernel

The host owns exactly this, and no more:

- the application window, the always-present top menu bar, and the dock area;
- the plugin loader and the plugin manager;
- configuration loading and validation;
- the `rola` capability service (`IRola`);
- i18n aggregation, theming, and logging;
- the open pipeline.

Anything that can be a plugin is a plugin. The two exceptions are the **plugin manager** and the
**log**, which are kernel because the program must remain diagnosable and recoverable.

### 3.2 Contract assembly

All extension points are declared in one assembly, `RorolalaDesktop.Contract`. Both the host and
every plugin reference it. The contract assembly is the only interface between them; the host does
not expose its own internal types.

Because plugins manipulate Avalonia controls directly (Section 3.3), the contract also references
Avalonia. The contract assembly version and the Avalonia version together form the **plugin ABI**
(Section 4.4).

### 3.3 Plugins

- Plugins are .NET assemblies (`.dll` on every platform).
- Plugins are loaded with a custom `AssemblyLoadContext`.
- Plugins **may manipulate Avalonia controls directly**. The contract does not hide the UI toolkit.
  This is a deliberate trade: it keeps the contract small and lets plugin authors write real UI,
  at the cost of coupling every plugin to the Avalonia version named by the contract.
- A plugin declares its identity, display-name key, contract version, and dependencies
  (Section 4.2).
- A plugin receives the host services through `IPluginHost` at initialization (Section 16).

### 3.4 Shared and private assemblies

The load context delegates the following to the host's default load context, so that there is
exactly one copy of each in the process:

- the contract assembly, `RorolalaDesktop.Contract`;
- the translations, `RorolalaDesktopI18n`: it holds the registered directories and the chosen locale
  as static state, so a plugin with its own copy would hold its own and be reading what nobody
  registered;
- the icons, `RorolalaDesktopSysIcons`, which asks the system for the picture it gives a file or a
  directory (Section 9);
- Avalonia and its satellites;
- the .NET base class libraries.

Everything else resolves from the plugin's own directory. What has to be understood about that
sentence is that **a plugin cannot depend on anything else**: only a plugin's own assembly and its
translations are laid beside it, so a dependency the host does not carry is a dependency that is
nowhere to be found at the moment the plugin is loaded. The list above is therefore the authority on
what a plugin may depend on, and adding a facility to it is done by having the host carry it — and by
naming it here. Resolving Avalonia or the contract privately would produce a second, incompatible copy
of those types; a `Control` built by such a copy is not the host's `Control` and cannot be inserted
into the host's visual tree.

## 4. Plugin Model

### 4.1 Discovery

- The host scans the `plugins/` directory beside the program executable for plugin assemblies.
- A plugin assembly contains exactly one type implementing `IRolaPlugin`.
- Discovery is by assembly, not by the configuration file. `plugins.json` records user state for
  plugins that have been discovered; it never lists where a plugin lives.

### 4.2 Identity and manifest

Every plugin exposes a `PluginManifest`:

| Field | Type | Meaning |
| --- | --- | --- |
| `Id` | `PluginId` | Stable identity. Globally unique. |
| `DisplayNameKey` | `string` | An i18n key; the host renders it to obtain the plugin's display name. |
| `ContractVersion` | `Version` | The contract version the plugin was compiled against. |
| `Dependencies` | `IReadOnlyList<PluginId>` | Other plugins that must be loaded first. |

`PluginId` grammar: `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$` — dotted snake_case segments, for
example `rorolala.file_system`. The identifier is permanent: it is written into user configuration
and into dock layout, so it must not change once shipped.

### 4.3 Assembly loading

- One `PluginLoadContext` is used for all plugin assemblies. A single context keeps a shared
  private dependency to one copy, so two plugins that depend on the same library see the same
  types. Per-plugin isolation is not a goal because plugins may depend on one another.
- The context delegates shared assemblies (Section 3.4) to the default load context and resolves
  everything else from the plugin directories.
- The host **does not support unloading** a plugin assembly in-process. Enabling, disabling, or
  reordering plugins takes effect on the next start (Section 4.6).

### 4.4 Contract and Avalonia version check

At load, the host compares:

- the plugin's `ContractVersion` against the host's contract version;
- the plugin's referenced Avalonia version against the host's Avalonia version.

On a mismatch the host **refuses to load that plugin** and reports it (Section 14.2). It does not
attempt to load it and fail later at first use.

### 4.5 Dependencies and load order

- Dependencies are declared by the plugin, never by the user.
- The host checks dependencies at startup:
  - a dependency that is not discovered, or is disabled, or failed to load;
  - a dependency cycle.
  Each is a **validation failure** and is fatal (Section 5.5).
- Load order is: dependency topological order first; within one dependency tier, the user `order`
  from `plugins.json`; ties broken by `PluginId` ordinal order, so the result is deterministic.
- A user ordering that contradicts the dependency order is **not fatal**. The host keeps the
  correct topological order and reports, in the plugin manager, "X must come after Y". This
  satisfies both requirements: the user is told who must follow whom, and a mere ordering mistake
  does not prevent the program from starting.

### 4.6 Lifecycle and reload

1. The host parses its own arguments (`-Lang:`, `-CurrentDir:`).
2. It loads and validates `preference.json` and `plugins.json`.
3. It discovers plugins and validates them against the configuration.
4. It computes the load order and loads the assemblies.
5. It calls `IRolaPlugin.Initialize(IPluginHost)` on each plugin in load order.
6. Plugins register extension points during initialization.
7. The host applies the look, resolves the language, and shows the window.

Enable/disable and order changes are written to `plugins.json` by the plugin manager and take
effect on the next start. There is no hot reload and no unload.

## 5. Configuration

### 5.1 Location

Both files live under the user's data directory, following the same convention as the `rola`
command line, whose user-level key directory is `rola/keys` under that root. On Linux this is:

```text
~/.local/share/rola/desktop/plugins.json
~/.local/share/rola/desktop/preference.json
~/.local/share/rola/desktop/theme.json
```

The exact root is whatever the platform's data-directory resolution returns; `dirs` semantics
apply, and `~/.local/share` is the Linux form only.

Every file carries a `_version`, and every one of them is fixed at `1` and read by no other: the
program is experimental, keeps nothing compatible, and a shape that changes is simply written by the
one writer of these files (§19.4).

### 5.2 plugins.json

`plugins.json` stores **user state only**. It does not list plugin paths, dependencies, contract
versions, or display names — those are declared by the plugin itself.

```json
{
  "_version": 1,
  "plugins": {
    "rorolala.file_system": { "enabled": true, "order": 0 },
    "rorolala.shelf":       { "enabled": true, "order": 1 }
  }
}
```

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `_version` | integer | yes | Schema version. Currently `1`. |
| `plugins` | object | yes | Map of `PluginId` to a state entry. |
| `plugins.<id>.enabled` | boolean | no | Whether the plugin loads. Defaults to `true`. |
| `plugins.<id>.order` | integer | no | User ordering within one dependency tier. Defaults to `0`. |

If the file does not exist, the host treats every discovered plugin as enabled with order `0` and
writes a default file. A missing file is not a validation failure; an unreadable or invalid one is.

### 5.3 preference.json

```json
{
  "_version": 1,
  "language": "zh-CN",
  "plugin": {
    "rorolala.file_system": { "Commands/move": "mv -v" }
  }
}
```

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `_version` | integer | yes | Schema version. Fixed at `1`. |
| `language` | string | no | Fallback locale, used only when `rola desktop` passes no `-Lang:`. |
| `plugin` | object | no | Per-plugin settings, keyed by `PluginId` and then by setting identity. The host interprets nothing beyond the value's own kind. |

If the file does not exist, the host uses the defaults above and writes it. A missing file is not a
validation failure; an unreadable or invalid one is.

A plugin reads its own section through `IPluginConfig.ReadKeyAs<T>(id)` (Section 16), where `id` is written
`Group/Key`: the whole of it is the key in the file, and the part before the slash is the group it is shown
under. A plugin also **declares** what its settings are through `IPluginConfig.Add`, which is what the
Preference dock shows (Section 7.4); a key that was never declared is still read from the file, so a section
written by hand still works. The program writes this file when the user changes a setting.

### 5.4 theme.json

```json
{
  "_version": 1,
  "mode": "system",
  "primary": "#B5E61D",
  "primaryText": "#1B2600",
  "accent": "#5CC8FF"
}
```

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `_version` | integer | yes | Schema version. Fixed at `1`. |
| `mode` | string | no | `system`, `light` or `dark`. `system` follows the desktop and goes on following it. Absent means `system`. |
| `primary` | string | no | The colour what is chosen is drawn in, as `#RRGGBB`. Absent means `#B5E61D`. |
| `primaryText` | string | no | What is written on a surface filled with the primary, as `#RRGGBB`. Absent means the look works it out by contrast (Section 10). |
| `accent` | string | no | The colour the drag marks are drawn in, as `#RRGGBB`. Absent means `#5CC8FF`. |

Every field is optional, and **a field that is not there is the default rather than a choice of it**
(Section 10). That is what makes taking a choice back possible: the panel's *Reset* removes the field
instead of writing the default down, so a default the program later changes is not kept out by a copy.
For the same reason a field is read as absent even when it spells the default out: what is in force is
the default either way, and only the file differs. `primaryText` is the one field where the absence
means something in its own right — nothing there is "work it out", which is not the same as any one
colour.

If the file does not exist, the host writes it with the defaults spelled out, so that the colours the
program is drawn in are there for a person to find and edit; a field removed afterwards stays removed.
A missing file is not a validation failure; an unreadable or invalid one is.

These four are the whole of what is configurable about how the program looks (Section 10), and they are
edited from the Preference dock under the kernel's own entry as well as by editing the file. It is a
file of its own rather than a section of `preference.json` because these are not a preference about the
program but what the program is drawn in, and nothing else belongs beside them.

### 5.5 Validation and failure

The following are **fatal**: the reason is written to standard error and the process exits with the
code in Section 5.6.

- `plugins.json`, `preference.json` or `theme.json` is unreadable, malformed, or not valid JSON;
- `_version` is absent or names a version the host does not support;
- a `PluginId` key is repeated, or names no discovered plugin;
- a declared dependency is missing, disabled, or forms a cycle;
- `theme.json` names a `mode` that is none of `system`, `light` and `dark`, or a `primary`, `primaryText`
  or `accent` that is not exactly `#RRGGBB`.

The following are **not fatal**:

- a user `order` that contradicts the dependency order (reported in the plugin manager);
- a plugin that fails the contract or Avalonia version check (that plugin is not loaded);
- a plugin that throws during initialization (that plugin is not loaded).

### 5.6 Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Clean exit. |
| `1` | `plugins.json` failed to load or validate. |
| `2` | `preference.json` failed to load or validate. |
| `3` | The configuration names a plugin that is not available. |
| `4` | `theme.json` failed to load or validate. |

The code is accompanied by a human-readable reason on standard error. No dialog is used: a failure
at this stage happens before the window exists.

### 5.7 Startup sequence

The order of Section 4.6 is normative. In particular the look is applied after the plugins and before
the window: after, because the run is assembled in one direction — the shell, then what the plugins
add to it, then how it is drawn — and before, because a style added once something has been styled
stops applying to it, and says nothing about it (Section 10).

## 6. Extension Points

### 6.1 Overview

A plugin receives `IPluginHost` at initialization and registers through it. The extension points
are:

| # | Extension point | Registered through |
| --- | --- | --- |
| 1 | Languages | `IPluginHost.I18n` |
| 2 | Context menus | `IPluginHost.ContextMenus` |
| 3 | Navigation buttons | `IPluginHost.Navigation` |
| 4 | Docks | `IPluginHost.Docks` |
| 5 | Open hooks and icon badges | `IPluginHost.OpenHooks`, `IPluginHost.IconBadges` |
| 6 | Top menu | `IPluginHost.Menu` |
| 7 | Settings | `IPluginHost.Config` |

Registration happens during `Initialize`. A plugin registered after the window is shown is not
supported.

### 6.2 Top menu

- The top menu bar is **always present**.
- The host provides:
  - `File / Open Directory`;
  - `Window /`, populated from the registered docks. A dock with open mode `Toggle` appears as a
    toggle; a dock with open mode `New` appears as a create entry.
  - `Window / Plugin Manager` (kernel, always present);
  - `Window / Log` (kernel, always present).
- A plugin may add top-level menus and items, addressed by a menu path. Menu labels are i18n keys.
- A plugin that injects into File System must declare a dependency on the File System plugin
  (Section 4.5).

### 6.3 Context menus

A plugin registers items for one or more of three contexts:

- **Directory** — right-click on a directory;
- **File** — right-click on a file;
- **Empty space** — right-click on empty area.

Each item carries a label key, an order, an optional icon, and a command invoked with the
`ContextTarget` that was right-clicked. Items from different plugins are ordered by plugin load
order and then by the item's order value.

### 6.4 Navigation buttons

A plugin may add buttons to the navigation area: a label or icon key, an order, and a command
invoked with the current directory.

### 6.5 Docks

See Section 7.

### 6.6 Open hooks

See Section 8.

### 6.7 Icon badges

See Section 9.

### 6.8 Languages

A plugin registers a translation directory (Section 11). It may also register additional locales
for a language picker.

## 7. Dock System

### 7.1 Registration

A dock is registered with a `DockRegistration`:

| Field | Meaning |
| --- | --- |
| `Owner` | The registering `PluginId` (the kernel for core docks). |
| `DockNameId` | Globally unique stable id, by convention `<plugin-id>.<dock>`. Used for layout persistence and for the `Window` menu. |
| `DisplayNameKey` | i18n key for the dock title. |
| `OpenMode` | `Toggle` or `New` (Section 7.2). |
| `DefaultPlacement` | The placement used when the dock is created without an explicit one (Section 7.2). |
| `Create` | Factory producing a new dock view for a requested placement. |

### 7.2 Open modes and placement

- `OpenMode.Toggle` — at most one instance exists. Activating it from the menu or the manager shows
  or hides it. The instance is keyed by `DockNameId`.
- `OpenMode.New` — every activation creates a new instance. File System uses `New`; this is what
  "copyable" means.
- `DockPlacement` is one of `Top`, `Left`, `Right`, `Bottom`, `Center`, `Float`.
- The default placement is declared for each creation: the factory receives the requested
  placement, which defaults to the registration's `DefaultPlacement`.

### 7.3 Instances and layout persistence

- Each live instance has a runtime handle assigned by the host. The handle is never written to
  configuration.
- Layout persistence uses the stable `DockNameId` plus an instance ordinal, so the set of open
  docks, their placements, and their sizes survive a restart.
- Because a `New` dock may have several instances, the persisted form records an ordinal per
  `DockNameId`.
- A dock may also **keep what it was** — the zoom it is reading at, what it has open — as text under its
  own keys, and the host writes it into the same record. It is one dock's state rather than the
  plugin's: two instances of one dock keep two sets of it, and a dock that is closed takes its own
  away. A key a dock never wrote reads as nothing, so a dock with nothing to remember is a dock that
  does not use this at all (`IDockState`, Section 16).
- A dock that keeps something while it is being restored keeps it in memory. What is written down is
  written when the docks are all there: a layout written mid-restore would be a layout with every dock
  further down the file dropped from it.

### 7.4 Core docks

| Dock | DockNameId | Open mode | Notes |
| --- | --- | --- | --- |
| Plugin Manager | `rorolala.core.plugin_manager` | Toggle | Kernel. Cannot be disabled. Enabled/disabled state and ordering of plugins are edited here. |
| Log | `rorolala.core.log` | Toggle | Kernel. Unity-style output at levels Trace, Debug, Info, Warn, Error (Section 12). |
| Preference | `rorolala.core.preference` | Toggle | Kernel. Every setting the owners declared, chosen by owner on the left and grouped as each owner grouped it on the right. Editing writes `preference.json` at once; a setting can be taken back to what it is declared to be, **by taking its value away rather than by writing the default down**, so that a later change to the declaration is not kept out by a copy of the old one; one that needs a restart says so. |

All three are always available from `Window`.

### 7.5 Bundled plugin docks

| Dock | Plugin | Open mode | Content |
| --- | --- | --- | --- |
| Directories | `rorolala.file_system` | `New` | One directory's entries at a zoom that is kept with the dock (§7.3): a table of rows below 60%, tiles above it. The table's columns are the icon, the name, the permissions, when it was last written and how large it is — and the permissions column is left out on Windows, which does not carry what those letters say. A column is made wider or narrower by the grab between it and its neighbour, which is drawn as the grab between two regions is (§10), and the name is the column that takes what is left over, so the table is as wide as the dock whatever the columns are. Provides the default icon library and badge composition (Section 9). Owns the data shared with Shelf. |
| Folder Tree | `rorolala.file_system` | `Toggle` | The directories under the base, as a tree, with a button that roots it at the top of the platform. A step is read when it is opened, and offers no expander when there is nothing under it. A step is opened and closed by that expander alone; a click on a row goes to the directory it names, wherever on the row it lands. A step with steps under it also offers to close every one of them, and nothing offers to open them all. Placed at the left by default. |
| File System Navigation | `rorolala.file_system` | `Toggle` | Back, forward, up, refresh, and the address. The address is **read** as crumbs and becomes a **field** when it is clicked, with the whole path in it and chosen; `Return` goes there, and `Esc` or leaving the field gives the crumbs back. A path that is not a directory changes nothing rather than moving the browser, and says so. While it is typed into, the filesystem is asked what the path could be and the answers are listed under the field (§7.5). Placed at the top by default. |
| Shelf | `rorolala.shelf` | `Toggle` | Back, forward, up; directory settings; search. Its data is owned by the File System plugin. |

The File System plugin is a plugin, but it is shipped with the program and is enabled by default.

**The entries are laid out with room between them**: the tiles apart on both axes, the rows of the table
apart vertically, and the room below the last row as well. That room is not only looks — selection needs
there to be a place an entry is not, or a frame could never be begun (§7.7) — and it is left on the
**item** the toolkit's list holds each entry in rather than on what that item draws, because it is the
item's own area that answers the pointer: room left inside an item is room the item still covers. Room at
the sides of a row is not left at all, since it would carry the row's columns away from the headings
standing over them.

A tree is a dock of its own rather than a third layout of the directory dock. A tree is not another
way of reading one directory — it is a way of walking the ones under a place, and it is rooted at the
base, which the location is not — so the two belong on screen at once, and neither is a mode of the
other. What the directory dock's zoom decides is therefore whether the entries are read as a table of
rows or as tiles, and nothing else: there is one scale, and the arrangement follows from it, because a
grid of pictures too small to look at is a grid nobody asked for. The zoom is a slider in the dock's own
corner, and `Ctrl` and a turn of the wheel over the dock move it.

Every listing carries a second thing in the same bar, at the end opposite the zoom: a toggle for the
entries the platform hides — a name beginning with a dot, and on Windows also the attribute that marks
one. Whether it is on is kept with the dock (§7.3), as the zoom is. What it decides is held by the
**browser** rather than by the dock, because the listing is the browser's and two docks looking at one
directory must list the same thing; a dock opened while another has it on opens with it on. While
hidden entries are shown they are drawn **fainter** than the rest, because they are there to be found
rather than read alongside the others.

A tree reads a step when it is opened, and that is why what it offers is the closing of steps and never
their opening: opening every step at once would read every directory under the base, which is the one
thing a tree read a step at a time is arranged to avoid. Closing them all is the cheap direction, and it
is offered by every row that has steps under it.

### 7.6 Headers and the strip

Each region has one strip above its content, and one dock out of those open in the region is shown.
The strip is one row, left to right:

1. **The tabs.** One header per dock open in the region, each as narrow as its word. Choosing one
   shows it. The header of the dock being shown is marked by a class, not by a size (Section 10).
2. **The drag area.** The rest of the strip belongs to the dock being shown, and is what it is
   dragged by. It draws nothing and shows a move cursor; the tabs are too narrow to be the only place
   a dock can be grabbed.
3. **The header commands** the shown dock brought with it, if any.
4. **The close button**, at the far end, which closes the dock being shown.

Behaviour:

- **Dragging** a tab or the drag area with the left button moves the dock: every zone a drop could
  mean is drawn, the one the pointer is over is picked out in the accent, and the dock lands in the one
  it is let go of over (Section 7.2). A drag that never leaves the threshold is a click, and a click on
  a tab shows that dock.
- **Middle-clicking** a tab or the drag area closes the dock, without dragging anything.
- Closing a `Toggle` dock hides it; closing a `New` dock discards the instance (Section 7.2). Either
  way the region settles on another of its docks, or is empty and collapses its strip.

The File System has **one location** and **one base**. Every dock it opens is a view of the location;
the tree is rooted at the base. The browser docks differ in the layout they read the entries in and in
nothing else; the navigation dock shows the one address and walks the one history. Two docks cannot be
looking at two places, which is what makes an address mean one thing.

The **base** is the directory the tree is rooted at, and it is what keeps the tree usable: a place to
work in rather than the whole filesystem. It starts at wherever the program was run. It is set from a
directory's own menu, wherever that directory is seen, and setting it goes there as well — the tree is
a view of the place being worked in, and a base the browser is not in would show somewhere else. The
tree keeps the steps a user has opened while the location moves, because it is rooted at the base
rather than at the location.

There is therefore exactly one way the location changes, and five things that ask it to:

1. A path typed into the **address** and entered. What is typed may be relative or have a step in it;
   the location is held in full, so the address says where the browser actually went. The address is a
   field only while it is being typed into: at rest it is the path read as crumbs, so it is read rather
   than edited by accident, and a path that is not a directory is refused there rather than leaving the
   browser somewhere that cannot be read.
2. A directory **clicked in the tree**. The tree holds nothing but directories, so choosing one can
   only mean going there, and it happens on the first click rather than the second.
3. A directory **opened in a list**.
4. A directory **opened in a grid**.
5. **`..`**, the entry a listing puts before its entries, which goes to the directory holding the one
   being looked at. A listing has it only when there is one further up **and** the directory being looked
   at is not the base (Section 7.6): the base is the place a user works in, and a listing that offered a
   step out of it would offer a step out of the work. The other four ways still reach above the base.

All five go through the same call, which refuses anything that is not a directory rather than leaving
the browser somewhere that cannot be read. The address is the one a user can get wrong, so it is the
one that says so. The navigation dock's arrows walk the history that switching the location leaves
behind.

**The address completes what is typed into it**, by asking the filesystem: what is in the field is split
into the directory it is under and the beginning of a name, that directory is listed, and the entries
whose names begin with what was typed are offered under the field. It completes a name rather than
searches, which is what a filesystem can answer for at every keystroke. What is offered is what the
listing would show — the entries the platform hides are kept out while they are hidden — directories
come first and then by name, and there are never more than a score of them. The arrows walk the offers
without the caret leaving the field, `Tab` or a click writes one into the field, and a directory is
written with its separator after it so that a path can be walked a step at a time. `Return` commits,
whether what is committed was picked or typed.

**The top of the platform** is where the tree's *Root* button goes, and what it roots the tree at: the
root on Unix, which is a directory like any other, and the computer on Windows, which is where the
drives are chosen from. Windows has no path naming every drive at once, so that one place is held as
the empty path, which no directory can be, and the views name it rather than printing it. A drive
root's parent is the computer, which is how a drive list is left; the computer has no parent.

Switching the location in one dock therefore switches it in all of them, and the navigation dock is
shown and hidden on its own — closing a browser does not close it.

The zoom stays in the directory dock rather than moving with the navigation: how large the entries are
read is a property of the dock reading them, and so is whether that makes them rows or tiles. The
**tree** is the one view rooted somewhere else — at the base (§7.6) — and it is a dock of its own for
that reason.

### 7.7 Browser interaction

The two directory layouts — the table and the tiles — are one browser seen two ways, and they are
driven alike. Everything here belongs to the File System plugin, not to the host: the host supplies
the window and the docks and knows nothing of entries.

- **Selection.** Clicking an entry chooses it alone; `Ctrl` adds it to, or takes it from, the choice;
  `Shift` takes everything between the entry the pointer or keyboard last landed on and the one
  clicked, in the order the listing shows. Chosen entries are filled with the primary (§10). Clicking
  the **space around the entries** drops the choice, and dragging from there draws a **frame** — a band
  in the primary — and chooses every entry it covers as it is drawn. The entries are spaced apart so that
  there is such a space between them (§7.5). Right-clicking keeps an existing choice when the entry is
  already in it, so a menu can be opened on a set.
- **The keyboard.** Arrow keys step — in the table one row at a time, in the tiles one tile across and
  one row down, so the arrows mean where the eye goes rather than the next index. `Home` and `End` go
  to the ends of the listing, `PageUp` and `PageDown` by a screenful. `Shift` extends from where the
  keyboard last landed; `Enter` opens the entry it is on. Typing picks the next entry whose name starts
  with what is typed, and a run is forgotten a second after its last character. **Pressing the same
  character again walks the entries that begin with it** rather than looking for a name that begins with
  two of them, so a listing of similarly named things can be walked from the keyboard; typing quickly
  spells a longer name instead. The search always starts after where the last move landed and wraps, and
  the way up is never matched, being a place rather than a name. `Ctrl+A` chooses every entry.
- **Clipboard.** `Ctrl+C` copies the chosen entries and `Ctrl+X` cuts them; `Ctrl+V` pastes into the
  directory being looked at. The paths go on the **system clipboard** — as files and as text — so a
  copy can be pasted into another program and a copy taken in another program can be pasted here. An
  entry that was cut is drawn faded until the paste, and that paste **moves** it, while anything else
  is copied. A name already taken in the destination is left where it is and a free name is made
  beside it (`name (2)`), so a paste never overwrites.
- **Dragging to move.** Dragging chosen entries onto a directory moves them into it — and onto the
  listing's **way up**, which moves them into the directory holding the one being looked at, since that is
  the place the entry stands for. A drag can leave the program or arrive from another one: the toolkit's
  drag is used, and Avalonia 12's X11 backend carries XDND. The files are offered themselves as well as
  their paths written out, so a file manager receives files and a text field text. A move out to another
  program is finished by taking the originals away, while a move the program answers itself has already
  moved them (§19.6).

### 7.8 The file agent

Every file operation the File System performs — a paste, a drop, and the removal that finishes a move out
of the program — is handed to a program of its own rather than done in process: `rola-desktop-fs-agent`.
It is reached through the File System plugin and through nothing else, which is why it is laid inside that
plugin's own directory (`plugins/FileSystemPlugin/RorolalaFSAgent/`) rather than beside the Desktop
program. The host does not load it: plugin discovery reads `plugins/` itself and not what is under it, so
a program of this kind can live there without being taken for a plugin.

The plugin starts it with:

```text
rola-desktop-fs-agent -Command:"<program and its arguments>" -Type:"Copy|Move|RemoveDirs|RemoveFiles"
                     -Lang:"<locale>" (-Pairs:"from>to;from>to" | -From:"<path>" -To:"<path>")
```

- `-Command` is the program and its fixed arguments that carry the operation out; the parameters are
  named rather than run through a shell, so a path with a space in it survives. The command is the
  operation's implementation, which is what lets the same agent serve copy, move and removal. The File
  System's own commands are its settings (`Commands/copy`, `Commands/move`, `Commands/remove_dirs`,
  `Commands/remove_files`), and what they are until the user chooses otherwise is **this program's own
  file operations** — `rola fs-ops cp|mv|rm` (§20) — named by the bare name `rola` rather than by the
  path it was found at, so that what is kept is what a reader would type. A system that keeps its tools
  elsewhere, or a user who prefers another, says so in the Preference dock (§7.4) rather than in a build.
- `-Pairs` is a batch, so that a question about a name is put once for a batch and its answer can stand
  for the rest; `-From`/`-To` is the single-item shorthand. An item goes over on its own where a path
  carries a batch separator, so that a path can never be read as two things.
- **Conflicts.** Copy and Move land on `<to>/<name of from>`. Where that name is already taken, the agent
  asks a person, in a window of its own worn in the same look as the program (§10): **Replace** (remove
  what is there), **Skip** (run nothing for it) or **Rename** (a free name beside it), with "apply to the
  remaining N" to answer every remaining conflict the same way. Closing the window calls the whole run
  off. **A conflict that was never answered is never run**: the command would land on something already
  there, so an unanswered conflict is reported as a failure rather than resolved by a default.
- **The answer.** One JSON line is the last thing on standard output — nothing else is written there —
  naming what became of every item, in the order they were given: `done`, `skipped` or `failed`, how it
  was resolved (`as-is`, `replaced`, `renamed`, `skipped`, `failed`), and the reason when it failed.
  Standard error is for a person. The exit code is `0` when a run happened and `1` when the arguments
  were not a run at all.

## 8. Open Hook Pipeline

### 8.1 Request state

An open is performed on an `OpenRequest`:

| Field | Meaning |
| --- | --- |
| `Target` | The entry being opened (path, kind). |
| `Verdict` | `Continue` or `Reject`. |
| `RejectReasonKey` | i18n key explaining a rejection. |
| `State` | A mutable bag for hooks to pass values forward. |

### 8.2 Stages and order

The pipeline runs in stages, in this order:

1. **CanOpen** — each hook may reject.
2. **BeforeOpen** — each hook may transform the request or reject.
3. The host performs the open.
4. **AfterOpen** — each hook is notified; it cannot reject.

Within a stage, hooks run in plugin load order (Section 4.5). A hook receives the request as
produced by its predecessor, and returns the request to pass on. A hook may therefore change what
later hooks see.

### 8.3 Rejection

- A hook rejects by setting `Verdict = Reject` (or returning a null request).
- Rejection is **final**: the open does not happen, and no later hook may overturn it. The pipeline
  stops.

### 8.4 Exceptions

- A hook that throws is **skipped**. The request passes to the next hook exactly as it was received
  by the failed hook — never half-modified.
- The host records the failure, names the skipped plugin, and raises a popup (Section 14.3).
- A thrown exception is **not** a rejection. The behaviour is deliberately fail-open: a broken hook
  does not block the user.

### 8.5 After-open

The AfterOpen stage is a notification. It cannot reject and its returned request is ignored; it
exists for plugins that react to a completed open.

## 9. Icon Badges

- The File System plugin provides the default icon library and composes the final icon. An entry is
  drawn with the icon **the system gives it** rather than one the program ships: a folder icon is the
  desktop's to draw, and one shipped here would look wrong on every desktop but the one it was drawn
  for. The library that reads them is `utils/desktop-sys-icons`; what it does per system is:
  - **Linux** — a freedesktop desktop, read through its icon theme: the theme named by the desktop,
    then what that theme inherits, then `hicolor`. A theme keeps a folder as an SVG more often than as
    a PNG (Papirus, Adwaita and Breeze all do), so the system's own renderer draws it, at the size
    asked for, once.
  - **Windows** — the shell, asked about the kind of thing rather than about a path, so every folder
    has the same picture and nothing has to exist on disk.
  - **Anywhere else, and wherever a system has nothing to give** — a mark the File System plugin draws
    itself, which says which of the two kinds a row is.
  What is not there yet is a picture per kind of file: a file is drawn as a file, not as a kind of one.
- A plugin contributes badges through `IIconBadgeProvider`, which has two methods:
  1. `Cares(Entry entry)` — a fast, convention-based check of whether the plugin has anything to
     say about this entry. The result is cached.
  2. `GetBadge(Entry entry)` — the badge to add, as a position and an icon key. The result is
     cached.
- Badges from several providers are placed by **offset**, in provider order. Overlap and overflow
  are permitted; no clipping rule is imposed.
- Caches live for the session and are invalidated on restart. Because a restart is also how plugins
  are reloaded, this is sufficient.

## 10. Theming

- `SimpleTheme` is **always loaded as the base theme**. Avalonia's controls have no template without one.
- The base is Simple rather than Fluent, deliberately. Fluent is a whole look, and an overlay on it
  spends half its rules overriding what Fluent had already decided — its resources, its rounded
  templates, its accent family. Simple decides little, so a theme on top of it says what it means. What
  that costs is that Simple draws little of its own: a hover it does not draw is a hover nobody draws,
  and what the design below does not state is left plain.
- The look is **one thing and it is not extensible**. There is no theme extension point: a plugin
  cannot supply one, and the look is not chosen by id. `RorolalaTheme` is applied as an **overlay** on
  top of the base, and it is the only overlay there is.
- What a run chooses is the **variant** it is drawn in and the **colours** it is drawn with — two, and a
  third that may be named rather than worked out (Section 5.4). Everything else in this section is stated
  rather than configured, which is what makes two runs of the program look like one program.
- The look is the one the sibling project **`gattipage`** uses, and the tokens are that project's own:
  three grounds, a ramp of ink in three steps, two border weights, 8- and 5-pixel radii, a soft
  two-layer shadow, and one vivid primary. Surfaces are **raised** rather than flat, so a dock reads as a
  card lying on the ground beneath it.
- The colours are spent by **role**, not by taste:
  - **Primary carries the weight.** It fills what is chosen — a chosen row is a wash of it, and a chosen
    tab and the one action of a surface are filled with it — and it is written into the base theme's own
    accent resources, so that a chosen row, a checked box and a selection of text are the primary from
    one definition.
  - **Accent is the second colour**, spent only on the marks a drag draws: the zone a dragged dock is
    aimed at, and the hairline a splitter shows under the pointer. They must never be mistaken for a
    selection, which is the whole reason there are two. The design this comes from needs no such colour;
    it is the one thing here that is not that design's own.
- The design:
  - **Rounded.** A card is rounded 8, a control 5, and a badge or a progress bar is a pill. Nothing is
    square but a rectangle too small to round.
  - **Three grounds, and nothing else.** Content sits on `bg`; chrome — the menu bar, a dock's strip, a
    toolbar — and a control sit on `bg-elevated`; what is hovered, held or welled sits in `bg-sunken`.
    The window is `bg` itself, so the bands and cards on it read as things laid over a ground.
  - **Two border weights.** One pixel of the border colour is every edge; under the pointer a control's
    edge firms up to the strong one. There is no third weight, and a control casts no shadow.
  - **A rule instead of a gap.** A table row is separated from the next by a one-pixel rule rather than
    by space, and the rows of a listing share the card's edge, clipped to its corners.
  - **The one action is filled.** The single action a surface exists for — the OK of a dialog, and
    nothing besides — wears the class `primary`: filled with the primary, in the ink the file names for
    it. There is at most one per surface, so that what to do is never a question with two answers.
  - **A chosen row is a wash, and its text is not recoloured.** The primary at 35 % on the light ground
    and 30 % on the dark one, with the words keeping the ink they had: a row filled solid would make a
    table read as a grid of buttons.
  - **A frame is the primary too.** The band drawn over a frame-selection is one pixel of the primary
    around a wash of the same colour, rounded like a control. It is a thing being chosen rather than a
    mark a drag draws, so it takes the primary and leaves the accent alone.
  - **A hover is the sunken ground** — not a tint and not a colour, which is what lets a hover and a
    chosen row sit side by side without ever being confused for one another.
  - **A tab strip is a segmented control.** One bordered rounded container with the tab being shown
    filled with the primary. There is no underline anywhere in the program.
  - **Nothing moves.** A mark that appears on selection reserves its space while it is not there, and no
    state change alters a position or a size.
- **The scale is fixed, and everything is on it.** Space is `4, 8, 12, 16, 24, 32` and nothing between: a
  row, a field and a control are **30** high, and a band of chrome — the menu bar, a region's strip, a
  toolbar — is **36**. A gap inside a block is 4 or 8, between blocks 24, and a panel is padded 16 across
  and 24 down. A number that is not on the scale is a number that drifts.
- **Text is a role, not a size written where the text is.** A **title** is 16 and semibold, body is 14, a
  **caption** is 12.5, and a **label** is 11, bold and letter-spaced. A role carries no colour but the
  neutral: **muted** is the middle step of the ink ramp and **faint** the last one, which is what a
  secondary line and the smallest print take — rather than an opacity written at the call site. A surface
  says which of them a piece of text is by putting the class on it (below), so that the same word looks
  the same wherever it appears.
- **A label is written in capitals**, because the design sets one that way and Avalonia has no
  `text-transform`: the caller passes the word already in capitals. A locale without case is unaffected,
  which is the right outcome rather than a cost.
- **A column of data is set in a monospace** (the `mono` class), because a table is read down its
  columns: the log is the one place this appears today, and it is what makes its columns line up.
- **A severity is not a colour the user chooses.** A failure is the design's own red and a warning its
  own amber, both fixed and both chosen to carry on either ground. A notice is raised as one card with a
  heading, a stripe per line in the colour of its level, and one filled way out — rather than as a stack
  of dialogs, which is the one shape a notice must not have.
- **Nothing is ever blank.** A list, a tree, a log or a panel with nothing in it says so: one muted line
  and a large faint glyph, in the middle of where the thing would have been. An empty surface that says
  nothing cannot be told from one that failed to load.
- The type is **the platform's own**, as the design uses it. The program ships no face, so two machines
  showing the same colours are the same program either way.
- **The ink on the primary** is the one the file names if it names one, and black or white by contrast
  ratio if it does not. The design names its own — a green-black rather than a plain black on a lime —
  which is why it is a field rather than a rule.
- **Motion** is colour and opacity only, never position or size, and everything takes **120 ms**: a hover,
  a press, a drop zone lighting up, a field taking focus. There is no longer move; a chosen row arrives
  with the rest of them (Section 17).
- The overlay is applied before the window is made. That order is load-bearing rather than tidy:
  growing `Application.Styles` after elements have been styled makes the base theme's setters win on
  those elements on the re-application that follows, so an overlay added late stops applying to
  everything already on screen, and says nothing about it.
- The shell marks the surfaces it owns, so that the look addresses them without knowing what a dock
  is. A plugin drawing into one of those surfaces may take the same marks:

  | Class | On |
  | --- | --- |
  | `menu-bar` | The menu bar. |
  | `dock-headers` | A region's header strip. |
  | `dock-tabs` | The segmented control a region's tabs sit in. |
  | `dock-title` | A dock's tab. |
  | `selected` | The tab of the dock the region is showing, in addition to `dock-title`. |
  | `dock-close` | The button that closes the dock a region is showing. |
  | `dock-splitter` | The grab between two regions, and the grab between two of a table's columns: four pixels wide. |
  | `dock-splitter-columns` / `dock-splitter-rows` | The same grab, saying which way it resizes. |
  | `dock-drop-zone` | Where a dragged dock would land. |
  | `dock-drop-target` | The zone a dragged dock is being aimed at, in addition to `dock-drop-zone`. |
  | `primary` | The one action a surface exists for: filled with the primary, and at most one per surface. |
  | `tool` | A square 28-pixel button for the chrome: it draws nothing of its own until the pointer is on it. |
  | `ghost` | The same, with a word in it. |
  | `label` / `title` / `caption` | Text roles: 11-pixel capitals, 16-pixel semibold, and 12.5-pixel. |
  | `muted` / `faint` | Secondary and faintest text: the middle and the last step of the ink ramp. |
  | `mono` | A column of data, in the monospace face. |

  A drop zone is drawn where the region it stands for is: that region's band of the area, as wide or as
  tall as the layout remembers for it and never less than the least a region may become. What a drag
  shows is therefore where the dock will be, and a region that is empty and taking no space still has a
  box to aim at. Every zone is up while a drag is on, drawn as a card on the sunken ground. The one the
  pointer is over carries `dock-drop-target` as well and is drawn in the accent: the others are the
  question, which regions there are to land in, and that one is the answer. The dock area asks the look
  for nothing here — the boxes are the look's to draw.

  A splitter draws nothing until the pointer is on it, and then a hairline of the accent through its
  middle. The grab is four pixels wide, which is what it has to stay for a hand to find it, and a line
  that wide would be a bar; so the hairline is drawn inside the grab rather than being it, and the
  class says which way it runs. A look that styles neither of the two classes leaves the splitter a
  bare grab with nothing to see, which is what the base theme alone does.

  A table's columns are moved by the same grab: the same classes, the same four pixels, the same
  hairline. The two are one thing to look at and one thing to drag, and a grab between two columns
  drawn as anything else would be a second answer to a question this section has already answered.

- The look is **applied once, before the window is made**: the styles are added to
  `Application.Styles` before there is a window, and never after it. A change made in the Preference
  dock takes effect **at once** nevertheless, because what a colour change replaces is a **resource**
  rather than a style: every control that took the resource is told it changed, and nothing is added to
  a list that must not grow.

## 11. Internationalization

- The host reads translations in the format already used by the command line: YAML files under a
  directory, one node per key, each leaf a locale-to-form mapping, `%{name}` placeholders filled by
  position. `RolaI18N` in `utils/desktop-i18n` is the existing reader.
- `RolaI18N` is extended from one translation directory to **several**:
  - the host registers its own directory first;
  - each plugin registers its own directory during initialization, in load order.
- Merge rule: **first registration wins**. A key already supplied is not replaced by a later
  directory.
- Every plugin key is namespaced. The prefix is the plugin's `PluginId` in snake_case, that is, the
  dotted id with `.` replaced by `_`. For `rorolala.file_system` the prefix is
  `rorolala_file_system`, and the plugin's display name key might be
  `rorolala_file_system.name`. The prefix is a writing convention that prevents collisions; it is
  not the merge mechanism.
- Locale fallback is `en`. A key that no file states, or that is stated in no language the program
  speaks, reads as the key itself.
- The language is chosen by the command line and passed to the program as `-Lang:`. When the
  program is started on its own, `preference.json`'s `language` is the fallback.

## 12. Logging

- The kernel provides a Log dock (`Window / Log`) with five levels, ordered Trace, Debug, Info,
  Warn, Error.
- Plugins log through `IPluginHost.Log`.
- The Log dock displays entries with their level, source plugin, message, and repeat count. It is a
  view over the host's log, not a second logging system.
- Log entries and popups share one deduplication service (Section 14.3): a repeated notification
  increments a count instead of adding a line or a second popup.

## 13. Rorolala Capability Injection

- The host exposes `IRola` to plugins, implemented over the existing C ABI (`librorolala`), which
  is built from `app/ffi` over the `#[lazyffi]` exports.
- Plugins call `IRola`. They do not invoke the `rola` command line, and they do not reimplement
  Rorolala semantics.
- The initial surface, corresponding to what the C ABI exports today:
  - locate a Workspace; locate a Vault; create either;
  - locate, find, count, and read members and accounts;
  - run the `handshake` and `sync_all` actions;
  - start the Vault daemon;
  - read Workspace and Vault configuration.
- **Known gap:** file-level storage operations (`storage write-file`, `storage extract-file`,
  `pack`) exist only in the command line and have no C ABI export yet. A plugin that needs them
  must wait for those exports; this specification does not permit shelling out as a workaround.

## 14. Failure Model

### 14.1 Fatal failures

Fatal failures are those in Section 5.5. They write a reason to standard error and exit with the
code from Section 5.6. The program does not attempt to show a window, and it does not fall back to
a default configuration. This is deliberate: a silently ignored configuration error is worse than
a loud stop, and the files are plain JSON that a person can edit.

The known cost: the plugin manager, which is the natural place to repair plugin configuration, is
itself inside the program that refuses to start. Recovery is by editing the file, guided by the
reason on standard error.

### 14.2 Non-fatal failures

The following do not stop the program:

- a plugin failing the contract or Avalonia version check — it is not loaded, and its dependents
  are not loaded;
- a plugin throwing during initialization — it is not loaded, and its dependents are not loaded;
- a plugin contradicting the load order — the correct order is kept and the plugin manager reports
  who must follow whom;
- an open hook throwing — it is skipped (Section 8.4).

They are reported in the Log dock and, where the user must act, in a popup.

### 14.3 Popup and log deduplication

- The kernel has **one** deduplication service, shared by popups and by the Log dock. There is no
  second mechanism.
- A notification is identified by a **hash of its combined content**: level, source, message, and
  the values formatted into it.
- Identical content is deduplicated. A popup is shown once; the Log dock keeps one entry and
  increments its repeat count.
- Content that differs by even a little is a distinct notification: a new popup is shown and a new
  log entry is recorded.
- Distinctness is scoped to one session; a restart starts with an empty table.

## 15. Configuration Reference

### plugins.json

```jsonc
{
  "_version": 1,
  "plugins": {
    "<PluginId>": { "enabled": true, "order": 0 }
  }
}
```

### preference.json

```jsonc
{
  "_version": 1,
  "language": "zh-CN",                 // fallback only
  "plugin": {
    "<PluginId>": { "<Group>/<Key>": "<value>" }
  }
}
```

### theme.json

```jsonc
{
  "_version": 1,
  "mode": "system",      // system | light | dark, or absent for the default
  "primary": "#B5E61D",  // #RRGGBB, or absent for the default
  "primaryText": "#1B2600", // #RRGGBB, or absent to have the look work it out
  "accent": "#5CC8FF"    // #RRGGBB, or absent for the default
}
```

### Data directory

```text
<user data dir>/rola/desktop/plugins.json
<user data dir>/rola/desktop/preference.json
<user data dir>/rola/desktop/theme.json
```

## 16. Contract Reference

The following is the contract surface. Names are part of the contract assembly version.

```csharp
namespace RorolalaDesktop.Contract;

public readonly record struct PluginId(string Value);

public sealed record PluginManifest(
    PluginId Id,
    string DisplayNameKey,
    Version ContractVersion,
    IReadOnlyList<PluginId> Dependencies);

public interface IRolaPlugin
{
    PluginManifest Manifest { get; }
    void Initialize(IPluginHost host);
}

public interface IPluginHost
{
    ILog Log { get; }
    II18n I18n { get; }
    IRola Rola { get; }
    IPluginConfig Config { get; }
    IMenuRegistry Menu { get; }
    IContextMenuRegistry ContextMenus { get; }
    INavigationRegistry Navigation { get; }
    IDockRegistry Docks { get; }
    IOpenHookRegistry OpenHooks { get; }
    IIconBadgeRegistry IconBadges { get; }
}

public interface II18n
{
    void RegisterDirectory(string directory);
}

public interface IPluginConfig
{
    T? ReadKeyAs<T>(string id, T? fallback = default);
    void Add(PluginSetting setting);
}

public enum SettingKind { Text, Bool, Number, Choice }

public sealed record SettingOption(string Value, string LabelKey);

public sealed record PluginSetting(
    string Id,                 // "Group/Key"
    SettingKind Kind,
    string LabelKey,
    string? Default = null,
    int Order = 0,
    bool RestartRequired = false,
    IReadOnlyList<SettingOption>? Options = null);

public interface IRola { /* Section 13 */ }

public interface ILog
{
    void Trace(string message);
    void Debug(string message);
    void Info(string message);
    void Warn(string message);
    void Error(string message);
}

public enum ContextMenuTarget { Directory, File, EmptySpace }

public interface IContextMenuRegistry
{
    void Add(ContextMenuTarget target, ContextMenuItem item);
}

public sealed record ContextMenuItem(
    string LabelKey,
    int Order,
    Action<ContextTarget> Command);

public interface IMenuRegistry
{
    void AddTopLevel(string labelKey, int order);
    void AddItem(string menuPath, MenuItem item);
}

public enum DockOpenMode { Toggle, New }
public enum DockPlacement { Top, Left, Right, Bottom, Center, Float }

public sealed record DockRegistration(
    PluginId Owner,
    string DockNameId,
    string DisplayNameKey,
    DockOpenMode OpenMode,
    DockPlacement DefaultPlacement,
    Func<DockPlacement, IDockView> Create);

public interface IDockView
{
    Control View { get; }
    IReadOnlyList<DockHeaderCommand> HeaderCommands { get; }
    void Restored(IDockState state) { }
}

public interface IDockState
{
    string? Read(string key);
    void Write(string key, string value);
}

public enum OpenStage { CanOpen, BeforeOpen, AfterOpen }

public sealed class OpenRequest
{
    public required Entry Target { get; init; }
    public OpenVerdict Verdict { get; set; } = OpenVerdict.Continue;
    public string? RejectReasonKey { get; set; }
    public IDictionary<string, object?> State { get; } = new Dictionary<string, object?>();
}

public enum OpenVerdict { Continue, Reject }

public interface IOpenHook
{
    OpenStage Stage { get; }
    OpenRequest? OnOpen(OpenRequest request); // null => reject; throw => skip
}

public interface IIconBadgeProvider
{
    bool Cares(Entry entry);
    Badge? GetBadge(Entry entry);
}

public sealed record Badge(int Position, string IconKey);
```

## 17. Feel and Interaction Craft

Users operate Desktop in long, high-intensity sessions. **Feel is a first-class requirement**, not
polish applied at the end: response latency, keyboard flow, focus and selection behaviour, dock
resizing, scrolling, and the absence of surprise count as much as correctness does.

This section is deliberately a placeholder. The concrete criteria — what must be instant, what must
never move under the cursor, what the keyboard must always be able to reach — are to be worked out
**with the user**, and recorded here. Until they are, no decision may trade feel away for
implementation convenience without raising it explicitly.

Agreed so far:

- **Motion.** What a change of state is communicated with is colour and opacity, never position or
  size, and it takes 120 ms — a hover, a press, a drop zone lighting up, a field taking focus. Nothing
  moves under the pointer, and a mark that appears on selection reserves its space while it is not there
  (Section 10).

## 18. Non-Goals

- Desktop does not implement version-control semantics; those live in `rola` and in plugins.
- Desktop does not support in-process plugin unloading or hot reload.
- Desktop does not sandbox plugins. A plugin is trusted code in the host process.
- Desktop does not provide a neutral UI description layer; plugins use Avalonia directly.
- Desktop does not shell out to the `rola` command line to obtain capabilities.

## 19. Open Items

1. The look's design is settled as far as Section 10 states it — the `gattipage` language, colour-only
   motion at one duration, three grounds and two chosen colours — but it is the design's own first
   application to a docked desktop rather than a web page, and the density of a dock is the part most
   likely to want revising with the user.
2. The concrete feel criteria (Section 17), to be agreed with the user.
3. The contract assembly versioning policy: increment rule and compatibility range. Nothing is done
   about it yet, and by decision: the program is experimental, the one plugin that is built with it is
   built from this tree, and removing the theme extension point therefore moved no version. The host
   still compares `Major.Minor.Build` exactly, so a plugin built against any other revision is refused.
4. The JSON schema versioning policy for `plugins.json`, `preference.json` and `theme.json`, and
   decided for now: **every file's `_version` is fixed at `1`, and the host reads no other**. The
   program is experimental and keeps nothing compatible, so a number that counted shape changes would
   count changes nobody needs warning about; a shape that changes is written by the one writer of these
   files, under the same `_version`.
5. The persisted dock-layout file format (placement and sizes per `DockNameId` and ordinal).
6. **Dragging between programs is carried by the toolkit from Avalonia 12.** 11.3.22's X11 backend had
   no `IPlatformDragSource` and no XDND, so a drag could not begin on X11 at all — not to another program,
   and not within this one through the toolkit. The program was moved to Avalonia 12 for this reason, and
   the toolkit's drag is what §7.7 offers: dragging to and from another program works on X11, and the
   hand-rolled in-program drag is gone. What that move cost: the toolchain's SDK floor moved to .NET 10,
   because Avalonia 12's analyzers need a newer compiler than .NET 8's; `Avalonia.Diagnostics` is gone,
   there being no 12 of it; and the look, the contract's Avalonia-facing surface and the icon reader were
   compiled against 12 and needed no further change.

## 20. References

- `app/ffi` and `src/lib.rs` — the C ABI surface used by `IRola`.
- `utils/desktop-i18n` — `RolaI18N`, the translation reader to be extended for several directories.
- `utils/desktop-sys-icons` — `SysIcons`, which asks the system for the icon it gives a file or a
directory (Section 9).
- `app/cli/rola/src/cmd_desktop.rs` — how the command line starts this program and hands over the
  language and the current directory.
- `app/cli/rola/src/cmd_fs_ops.rs` — `rola fs-ops`, the file operations the agent of §7.8 runs.
- `AGENTS.md` — repository conventions, including the English rule for documentation.
