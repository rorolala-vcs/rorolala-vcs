//! Marking a directory hidden the way each platform says.
//!
//! A name beginning with a dot is the whole of the convention where this was written, and Windows is
//! not one of those platforms: it shows such a directory like any other, and has an attribute for the
//! answer instead. What is set here is that attribute, which is what Explorer and everything else on
//! the platform reads, so a Workspace's and a tree's own things look like the program's rather than
//! like the work's.

use std::path::Path;

/// What Windows calls a file or directory that is not shown.
#[cfg(windows)]
pub const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;

#[cfg(windows)]
mod platform {
    use std::os::windows::ffi::OsStrExt as _;
    use std::path::Path;

    use super::FILE_ATTRIBUTE_HIDDEN;

    /// What a call that answers with a path's attributes answers when it could not read them.
    const INVALID: u32 = u32::MAX;

    // SAFETY: `kernel32` provides both symbols under these signatures, and the module is behind
    // `#[cfg(windows)]`, so nothing else compiles the declaration; each call site carries its own
    // safety argument.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        /// Reads what a path's attributes are.
        fn GetFileAttributesW(name: *const u16) -> u32;

        /// Says what a path's attributes are.
        fn SetFileAttributesW(name: *const u16, attributes: u32) -> i32;
    }

    /// Tries to mark `directory` hidden, answering whether Windows took it.
    ///
    /// The read is there because the write is told the whole of what a path's attributes are rather
    /// than the one bit that is wanted: setting that bit alone would take away everything else the
    /// directory carries.
    pub fn mark_hidden(directory: &Path) -> bool {
        let mut name: Vec<u16> = directory.as_os_str().encode_wide().collect();
        name.push(0);

        // SAFETY: `name` is the NUL-terminated wide string both calls are documented to take, and it
        // outlives them both.
        let attributes = unsafe { GetFileAttributesW(name.as_ptr()) };

        if attributes == INVALID {
            return false;
        }

        // SAFETY: as above, and the attributes are this call's own answer with one bit added.
        unsafe { SetFileAttributesW(name.as_ptr(), attributes | FILE_ATTRIBUTE_HIDDEN) != 0 }
    }
}

/// Marks `directory` hidden where the platform has an attribute for it, answering whether it did.
///
/// It is a best effort: a directory whose attribute cannot be written is still the directory, and
/// what is inside it is still what it is. A platform without the attribute has nothing to answer.
#[must_use]
#[cfg(windows)]
pub fn mark_hidden(directory: &Path) -> bool {
    platform::mark_hidden(directory)
}

/// Marks `directory` hidden where the platform has an attribute for it, answering whether it did.
#[must_use]
#[cfg(not(windows))]
pub const fn mark_hidden(_directory: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::mark_hidden;

    /// A platform that has the attribute takes it, and one that does not says so rather than
    /// pretending the name already did it.
    #[test]
    fn marking_a_directory_is_answered_by_the_platform() {
        let dir = std::env::temp_dir().join(format!(
            "rorolala-hidden-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.subsec_nanos())
        ));

        fs::create_dir_all(&dir).expect("a scratch directory");

        let marked = mark_hidden(&dir);

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;

            assert!(marked);
            assert_ne!(
                fs::metadata(&dir).expect("the directory").file_attributes()
                    & super::FILE_ATTRIBUTE_HIDDEN,
                0
            );
        }

        #[cfg(not(windows))]
        assert!(!marked);

        let _ = fs::remove_dir_all(&dir);
    }
}
