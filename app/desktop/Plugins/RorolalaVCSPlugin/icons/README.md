# The RorolalaVCSPlugin pictures

The pictures the `RorolalaVCSPlugin` project draws: `lock`, `pencil`, `lock-open`, `warning-circle`, `warning`, `question`, `arrow-clockwise`, `eye`, `eye-slash`, `arrow-up`, `arrow-down`, `git-fork`, `git-merge`, `plus`, `note-pencil`, `arrow-right`, `exclamation-mark`, `cloud-arrow-up`, `cloud-arrow-down`, `arrows-clockwise`, `dots-three-vertical`, `copy`, `list-bullets`, `dots-six-vertical`, `minus-circle`, `code`, `record`, `scissors`, `file`, `x`, `arrows-in-line-vertical`, `tree`, `check-circle`, `radio-button`, `circle`, `list-checks`, `briefcase`, `arrows-split`, `user-circle-gear`, `plus-bold`, `note-pencil-bold`, `exclamation-mark-bold`, `arrow-right-bold`, `check-circle-bold`, `radio-button-bold`, `circle-bold`, `arrow-up-bold`, `arrow-down-bold`, `git-fork-bold`, `git-merge-bold`, `pencil-bold`, `lock-bold`, `trash-bold`.

They are Phosphor Icons, taken from the set's own repository,
[phosphor-icons/core](https://github.com/phosphor-icons/core), under the [MIT License](https://github.com/phosphor-icons/core/blob/main/LICENSE).
The license itself is `LICENSE`, copied from the commit below.

**The set is not vendored here.** It is a shallow clone under `THIRD_PARTY/phosphor-icons/`, which
git ignores, and the pictures are drawn from it by `cargo dev icons` rather than copied by hand, so
that what is drawn is what the pinned commit holds. The manifest is `app/desktop/icons.toml`:
another picture is another name there rather than another file in this directory.

- drawn from: `2b75f3ad12b420c9504ef05df8d2564a28f8500e`, at `regular`
- handed over under: `rorolala_vcs.*`

**Nothing here is edited by hand.** The pictures, this file and the license are what that run
writes; a change made here is a change made nowhere.

**Why white.** A picture is not drawn as a picture: what wears it draws it as an opacity mask over a
ground it colours from the theme, so the mark takes the ink the user is looking at rather than a
colour baked into the file. White is opaque under either reading of a mask, its alpha, and its
luminance, so the picture works whichever one the toolkit does.

**Why raster rather than linked.** Nothing at runtime draws an SVG: the toolkit would need another
package for it, and a plugin is discovered as a single assembly. The pictures are drawn at 96
pixels from the set's 256-unit viewBox, so the mask is scaled down rather than up.
