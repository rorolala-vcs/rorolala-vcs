# RorolalaDesktop

## Introduction

`RorolalaDesktop` is the desktop program of Rorolala: a dockable Avalonia shell that discovers the plugins beside it, honours their load order, and draws the docks, menus, icons and settings they register.

## Boundary

`Core` owns the shell: the main window, docking and layout, preferences, the look, logging and the single instance. `Host` is the program beside the shell that starts it, forwards the line saying the window is up, and reports a run that ends badly. `Contract` is the boundary between the host and its plugins, holding `IRolaPlugin`, `IPluginHost` and the registries, and its version is what a plugin's manifest is checked against. `Plugins` holds the four that ship with the program: File System, Git, Rorolala and Terminal. The shell does not own the rules those plugins apply: version control comes from `librorolala` behind the C ABI, and file operations run the user's own tools. The `IRola` interface is declared in `Contract` and bound in `Core`; until those bindings are written it states nothing. The command line starts this program as `rola desktop` and hands over the language, the starting directory, the frame style and the request to report that the window is up.
