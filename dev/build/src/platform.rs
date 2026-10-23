//! How a platform spells the artifacts a build produces.
//!
//! Cargo names a Unix shared library with a `lib` prefix and a Windows one with a `.dll` suffix, so
//! what a build hands over is normalised to one shape per platform. The table is written for every
//! platform rather than for the one it is compiled on, so what each name is can be checked by a test
//! on any machine — a spelling only the platform that never runs the tests would meet is the one
//! that goes wrong unnoticed.

/// The platform a build is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    /// Windows: a `.dll` shared library, a `.lib` static one, and the import library a DLL is
    /// linked through.
    Windows,
    /// macOS: a `.dylib` shared library, a `.a` static one, and no import library — a linker is
    /// given the shared library itself.
    Darwin,
    /// The other Unixes: a `.so` shared library, a `.a` static one, no import library.
    Unix,
}

impl Platform {
    /// The platform this build is made on.
    #[must_use]
    pub fn current() -> Self {
        Self::of(std::env::consts::OS)
    }

    /// The platform an operating system — a `std::env::consts::OS` value — builds for.
    #[must_use]
    pub fn of(os: &str) -> Self {
        match os {
            "windows" => Self::Windows,
            "macos" => Self::Darwin,
            _ => Self::Unix,
        }
    }

    /// The platform's name, as a reader would say it and as `.cargo/config.toml` is keyed by.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Darwin => "darwin",
            Self::Unix => "unix",
        }
    }

    /// The suffix a program's file name carries, or nothing where the platform has none.
    #[must_use]
    pub fn exe_suffix(self) -> &'static str {
        match self {
            Self::Windows => ".exe",
            Self::Darwin | Self::Unix => "",
        }
    }

    /// The name cargo writes the shared library as.
    #[must_use]
    pub fn cargo_shared(self) -> &'static str {
        match self {
            Self::Windows => "rorolala.dll",
            Self::Darwin => "librorolala.dylib",
            Self::Unix => "librorolala.so",
        }
    }

    /// The name cargo writes the static library as.
    #[must_use]
    pub fn cargo_static(self) -> &'static str {
        match self {
            Self::Windows => "rorolala.lib",
            Self::Darwin | Self::Unix => "librorolala.a",
        }
    }

    /// The name cargo writes the import library as, where the platform has one.
    ///
    /// A DLL is linked through the import library that names what it exports, and cargo writes that
    /// one under the DLL's own name — `rorolala.dll.lib` — so that it does not take the static
    /// library's.
    #[must_use]
    pub fn cargo_import(self) -> Option<&'static str> {
        match self {
            Self::Windows => Some("rorolala.dll.lib"),
            Self::Darwin | Self::Unix => None,
        }
    }

    /// The name the shared library is handed over as.
    #[must_use]
    pub fn shared_exported(self) -> &'static str {
        match self {
            Self::Windows => "rorolala.dll",
            Self::Darwin => "rorolala.dylib",
            Self::Unix => "rorolala.so",
        }
    }

    /// The name the static library is handed over as.
    #[must_use]
    pub fn static_exported(self) -> &'static str {
        match self {
            Self::Windows => "rorolala.lib",
            Self::Darwin | Self::Unix => "rorolala.a",
        }
    }

    /// The name the import library is handed over as, where the platform has one.
    #[must_use]
    pub fn import_exported(self) -> Option<&'static str> {
        self.cargo_import().map(|_| "rorolala.dll.lib")
    }
}

/// The operating-system part of an archive's name, following what `uname -s` printed when the names
/// were first laid down.
///
/// The archive a release carries keeps the name it had, so this is not the spelling
/// `std::env::consts::OS` gives: `macos` was `darwin`, and a Windows archive — which is never made,
/// a Windows hand-off being an installer — was `windows_nt`.
#[must_use]
pub fn uname_os(os: &'static str) -> &'static str {
    match os {
        "macos" => "darwin",
        "windows" => "windows_nt",
        other => other,
    }
}

/// The machine part of an archive's name, following what `uname -m` printed.
#[must_use]
pub fn uname_arch(arch: &'static str) -> &'static str {
    match arch {
        "aarch64" => "arm64",
        "x86" => "i686",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{Platform, uname_arch, uname_os};

    #[test]
    fn a_platform_is_read_from_an_operating_system_name() {
        assert_eq!(Platform::of("windows"), Platform::Windows);
        assert_eq!(Platform::of("macos"), Platform::Darwin);
        assert_eq!(Platform::of("linux"), Platform::Unix);
        assert_eq!(Platform::of("freebsd"), Platform::Unix);
    }

    #[test]
    fn the_artifacts_cargo_writes_and_the_ones_handed_over_are_named_apart() {
        // Unix: cargo's `lib` prefix is dropped on the way out, so a consumer links `rorolala`.
        assert_eq!(Platform::Unix.cargo_shared(), "librorolala.so");
        assert_eq!(Platform::Unix.shared_exported(), "rorolala.so");
        assert_eq!(Platform::Unix.cargo_static(), "librorolala.a");
        assert_eq!(Platform::Unix.static_exported(), "rorolala.a");
        assert_eq!(Platform::Unix.cargo_import(), None);
        assert_eq!(Platform::Unix.exe_suffix(), "");

        // Windows: a DLL is linked through the import library beside it, and a program carries
        // `.exe`.
        assert_eq!(Platform::Windows.cargo_shared(), "rorolala.dll");
        assert_eq!(Platform::Windows.shared_exported(), "rorolala.dll");
        assert_eq!(Platform::Windows.cargo_static(), "rorolala.lib");
        assert_eq!(Platform::Windows.cargo_import(), Some("rorolala.dll.lib"));
        assert_eq!(
            Platform::Windows.import_exported(),
            Some("rorolala.dll.lib")
        );
        assert_eq!(Platform::Windows.exe_suffix(), ".exe");
    }

    #[test]
    fn the_archive_names_keep_the_spelling_uname_gave_them() {
        assert_eq!(uname_os("linux"), "linux");
        assert_eq!(uname_os("macos"), "darwin");
        assert_eq!(uname_os("windows"), "windows_nt");

        assert_eq!(uname_arch("x86_64"), "x86_64");
        assert_eq!(uname_arch("aarch64"), "arm64");
        assert_eq!(uname_arch("x86"), "i686");
    }
}
