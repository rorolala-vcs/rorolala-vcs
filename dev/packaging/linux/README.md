# Linux packaging

What `cargo dev publish` lays a Linux hand-off down from. The code that stages the trees and writes
the packager's configuration is `dev/build/src/linux.rs`; this directory is the copy that the
configuration points at.

```text
rolad.service                  the user unit, installed but never enabled by the package
rorolala-daemon.desktop        ROLADaemon, running rola-daemon-gui
rorolala-launcher.desktop      ROLALauncher, in the system package
appimage.desktop               ROLALauncher, inside the AppImage
AppRun                         what the AppImage runs
scripts/refresh.sh             the desktop database and icon cache, and nothing about systemd
```

The two `.desktop` templates and `appimage.desktop` say `@COMMENT@` where the workspace's
`description` from `Cargo.toml` is written. Nothing else in them changes from one release to the
next.

`rolad.service` is a **user** unit. It reads the same `~/.local/share/rola` the command line and
`rola-daemon-gui` do, and it is deliberately not enabled or started by any package script: whether
the daemon runs is its user's decision. The package installs the unit at
`/usr/lib/systemd/user/rolad.service`, and a user starts it with:

```sh
systemctl --user daemon-reload
systemctl --user enable --now rolad.service
```

`refresh.sh` is the only script a package runs. It refreshes the desktop database and the icon
cache when the tools are there, and does nothing else; in particular it never touches systemd and
never deletes the user's data.
