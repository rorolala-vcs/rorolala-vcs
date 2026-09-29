//! Build script: link the program's own icon into the Windows executable.
//!
//! A Windows program's icon is a resource linked into it rather than something it draws, so the
//! script beside the generated icon is compiled here and linked in by the directive the resource
//! crate adds. Nowhere else is there such a resource or a compiler for one, so nowhere else is
//! there anything to do — which is why this is decided by the host rather than by the target.

/// The resource script naming the program's icon, relative to the manifest directory.
///
/// The script belongs beside the icon it names, which the workspace build script makes from the
/// description beside it — `res/icons/rola-cli.png.toml` — and names it bare, because the resource
/// compiler reads a name in a script relative to the script rather than to where it was run from.
#[cfg(windows)]
const ICON_SCRIPT: &str = "../../../res/icons/rola-cli.rc";

/// The icon [`ICON_SCRIPT`] names, relative to the manifest directory, watched so that a picture
/// that was made again is linked in again.
#[cfg(windows)]
const ICON: &str = "../../../res/icons/rola-cli.ico";

fn main() {
    #[cfg(windows)]
    {
        // Both are watched: the icon is made from the picture beside it by the workspace's build
        // script, and a picture that changed would otherwise leave the old icon linked in.
        println!("cargo:rerun-if-changed={ICON_SCRIPT}");
        println!("cargo:rerun-if-changed={ICON}");

        embed_resource::compile(ICON_SCRIPT, embed_resource::NONE)
            .manifest_optional()
            .expect("the program's icon could not be linked in");
    }
}
