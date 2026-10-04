# Lock pictures

The two pictures the Ownership column and a locked tile are drawn with: `lock`, which says an entry
is held by somebody else, and `lock_open`, which says it is held by the reader — whose it is to let
go of.

They are Google's Material Design icons, taken from the maintained mirror of the set,
[material-icons/material-icons](https://github.com/material-icons/material-icons) — the same artwork
as [google/material-design-icons](https://github.com/google/material-design-icons) — under the
[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0).

**The set is not vendored here.** It is a shallow clone under `THIRD-PARTY/material-icons/`, which
git ignores, and the pictures are drawn from it by `./run.sh icons` rather than copied by hand — so
what is drawn is what the pinned commit holds, and another picture is another name on that script's
list rather than another file copied into this directory. The commit is named in the script
(`ICONS_COMMIT`); the upstream is archived, so it is expected to stay where it is.

The `.png` files are what the plugin embeds, and they **are** committed: a build needs neither the
clone nor a renderer, only the regeneration does. `export` runs the script first, so what ships is
always drawn from the pin.

**Why white.** A picture is not drawn as a picture: the File System plugin draws it as an opacity
mask over a ground it colours from the theme, so the mark takes the accent or the error red the user
is looking at rather than a colour baked into the file. The script sets that ink on the way through;
a white glyph is opaque under either reading of a mask — its alpha, and its luminance — so the
pictures work whichever one the toolkit does.

**Why raster rather than linked.** Nothing at runtime draws an SVG: the toolkit would need another
package for it, and the plugin is discovered as a single assembly. 96 pixels is four times the
24-unit viewBox the artwork is drawn at, which is as large as a tile's icon ever gets at the top of
the zoom, so the mask is scaled down rather than up.
