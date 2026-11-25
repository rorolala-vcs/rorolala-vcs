# Rorolala Desktop panel prototype template

## Introduction

One HTML file that draws a single Rorolala Desktop dock panel at 1:1, as the starting point for a
Desktop interface prototype. The panel is the dock frame and its header strip, with one selected tab
and an empty content surface. The rest of the page is the prototype tool: the theme switch, the
palette editor, the motion levels and the zoom.

Copy it; do not edit it in place. Every later prototype is copied from it, so a change here changes
all of them.

## Copy it

```sh
cp dev/tmpls/desktop-prototype/index.html __PLAN_<name>/__prototype/index.html
```

In the implementation phase the location is `__ui_prototype_<name>/index.html`. The `ui-design`
skill states the two phases.

Rename the tab and put the design in `.content`. Keep the chrome, the tokens and the tool.

## What is 1:1

The panel restates the Desktop's own numbers and colours:

- the frame: square, a 1px `--splitter` border, the `--bright` face;
- the strip: 32px tall, the `--dark` (sunken) face, a 1px bottom line;
- the tab: a 40px leading room, a 16px mark 18px from the tab's edge, the name in the middle, a
  26px close room, a 14px name, SemiBold when selected, the `--bright` face joined to the panel by
  covering the strip's line;
- a single tab's edges: no left line and one right line, as the strip of one dock is drawn;
- the close: `✕`, one strip of the tab's right side, drawn under the pointer;
- the drag area and the commands slot: the rest of the strip.

The stage behind it is a quiet square grid, drawn from the splitter colour; it is a preview aid and
not part of the panel.

The palette defaults are `ThemePalette.DefaultLight` and `ThemePalette.DefaultDark`, and the
semantic states are `SemanticPalette.Dark` and `SemanticPalette.Light`. The derived tokens are the
look's own:

- `Muted = Mix(Front, Mid, 0.38)`, `Faint = Mix(Front, Mid, 0.62)`,
  `Strong = Mix(Splitter, Front, 0.25)`;
- `TabFill = Mix(Dark, Bright, 0.5)`;
- the hover wash is black at 8% on a light ground and white at 8% on a dark one;
- the selection wash is the primary at 36/255 on light and 48/255 on dark;
- the focus ring is the primary at 140/255, 2px out;
- the shadows and the washes are the look's values in `RorolalaTheme`.

The motion durations are the Desktop's, from `Motion.cs`: feedback 120ms, state 160ms, panel 220ms
and bar 280ms, with an ease-out cubic. `Reduce` keeps the fades and zeroes the travel; `None` zeroes
every duration.

## What the tool does

- `System`, `Light` and `Dark`: the first follows the desktop's colour scheme and is the default.
- The palette drawer shows the active theme's five grounds, the two configured colours and the
  twelve semantic states. Each row has a colour input and a restore button. The two buttons at the
  foot restore the active theme or every colour. The configured colours are one pair for both
  themes, as they are in the Desktop.
- `Full`, `Reduce` and `None`: the motion level. The first load follows the desktop's
  reduced-motion setting when nothing is stored. `Test` plays a small sample in the level in force.
- The zoom runs from 50% to 200% in 5% steps. 100% is 1:1 in CSS pixels.
- Drag the panel's edges or corners to resize it. The drag follows the zoom, and the size is stored
  with the other choices.
- Every choice is stored under `rorolala.desktop-prototype.v1` in this browser. The restore
  buttons write the Desktop's palette defaults back into the state; the theme, motion, zoom and
  panel size are separate choices.
- The tab's close hides the panel and `Show panel` brings it back, so the enter and exit motion can
  be seen.
- `Palette` opens the drawer; `Escape`, the scrim and the close button close it.

## What to replace

- The default `info` mark in the tab's leading room: replace the `--info-icon` data URI with the
  panel's own picture, or replace the `.tab-icon` element with an inline mark. Draw the picture from
  `icons.toml`.
- The `.tab-name` text, the tab's `aria-label` and the content's `aria-label`.
- The empty `.content` with the design.
- The empty `.commands` slot with the panel's header commands.
- The panel's starting size, `--panel-w` and `--panel-h`, only if the prototype needs another
  default; the drag changes it at run time. The chrome values, the tokens and the tool stay.

## Fidelity notes

- The panel is square: the design has no rounded corners, so `--radius` and `--small` are 0 and the
  tool's controls are square too.
- The tab's mark is the picture the app's tab carries: a white picture cut out and painted in the
  tab's ink (`Marks.Tab`). The template embeds the Sight plugin's `info.png`, 96px, as a mask and
  paints it with the tab's ink. Replace it with the panel's own picture from the icon manifest.
- The app draws a strip of one dock as a title, not a tab (`DockArea.SingleClass`): a title at the
  strip's 12px inset, 13px SemiBold, with no tab chrome. This template shows the selected-tab shape
  so a panel's tab is visible. A future design with one tab may switch to the title shape when that
  is the state it is in.
- The app trims a long tab name from the front, so a path keeps its tail
  (`TextTrimming.LeadingCharacterEllipsis`). The template uses a trailing ellipsis. A tab named by
  a path should follow the app's rule.
- The panel carries the look's raised shadow. The app uses it for the lifted dock a drag carries,
  and a dock standing in the window is flush. The stage is a preview, so the template raises the
  panel to stand off the ground.
- The tab closes the panel in this template. In the app it closes the dock. A future prototype can
  keep it or remove it.
- A tab's name is capped at 240px, as the app caps it.

## Where each value comes from

- `app/desktop/Contract/ThemePalette.cs`: the light and dark five, the primary and the accent.
- `app/desktop/Contract/SemanticPalette.cs`: the twelve states.
- `app/desktop/Core/Theming/RorolalaTheme.cs`: the derived inks, lines, washes, shadows and the dock
  styles.
- `app/desktop/Core/Docking/DockArea.cs`: the strip height, the tab rooms and the leading and
  trailing edges.
- `app/desktop/Core/Theming/Motion.cs`: the durations and the ease.
- `app/desktop/Core/CoreDocks/PreferenceDock.cs`: the colour rows, the mode and motion choices and
  the restore.
- `app/desktop/Plugins/SightPlugin/icons/info.png`: the default tab mark, drawn from the Phosphor
  `info` picture. `app/desktop/icons.toml` is the manifest a panel's own picture is added to.

When one of those changes, change the template with it. A template that drifts from the program is
a prototype that lies about it.
