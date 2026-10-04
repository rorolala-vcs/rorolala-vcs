# The FileSystemPlugin pictures

The pictures the `FileSystemPlugin` plugin draws: `visibility`, `visibility_off`, `link`, `link_off`, `arrow_back`, `arrow_forward`, `arrow_upward`, `refresh`, `launch`, `file_copy`, `content_cut`, `content_paste`, `delete`, `local_offer`, `create_new_folder`.

They are Google's Material Design icons, taken from the maintained mirror of the set,
[material-icons/material-icons](https://github.com/material-icons/material-icons) — the same artwork as
[google/material-design-icons](https://github.com/google/material-design-icons) — under the
[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0). The license itself is
`LICENSE`, copied from the commit below.

**The set is not vendored here.** It is a shallow clone under `THIRD-PARTY/material-icons/`, which
git ignores, and the pictures are drawn from it by `./run.sh icons` rather than copied by hand, so
that what is drawn is what the pinned commit holds. The manifest is `app/desktop/icons.toml`:
another picture is another name there rather than another file in this directory.

- drawn from: `af0ed9c0e1276bad43c4d6ca8e8aaa283e425195`
- handed over under: `rorolala_file_system.*`

**Nothing here is edited by hand.** The pictures, this file and the license are what that run
writes; a change made here is a change made nowhere.

**Why white.** A picture is not drawn as a picture: a plugin draws it as an opacity mask over a
ground it colours from the theme, so the mark takes the ink the user is looking at rather than a
colour baked into the file. White is opaque under either reading of a mask — its alpha, and its
luminance — so the picture works whichever one the toolkit does.

**Why raster rather than linked.** Nothing at runtime draws an SVG: the toolkit would need another
package for it, and a plugin is discovered as a single assembly. The pictures are drawn at 96
pixels, which is four times the 24-unit viewBox the artwork is drawn at — as large as a tile's icon
ever gets at the top of the zoom, so the mask is scaled down rather than up.
