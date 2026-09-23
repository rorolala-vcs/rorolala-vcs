//! The `rola key generate` command: make a key pair under a name.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use librorolala::auth::user_keys_dir;
use mingling::{
    Grouped,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::{PRIVATE_KEY_EXTENSION, PUBLIC_KEY_EXTENSION};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_KEY_ARGUMENT, EC_ERR_KEYGEN_FAILED, EC_ERR_KEYGEN_INSTALL_FAILED,
    EC_ERR_KEYGEN_NO_KEY_DIR, EC_ERR_KEYGEN_NO_OPENSSL, EC_HELP,
};
use crate::failure::failure;

/// The algorithm a key pair is generated for.
///
/// Ed25519 is the only one this build knows, and the one the rest of Rorolala reads: what
/// `openssl genpkey` writes for it is a PKCS#8 PEM, which is exactly what an account's
/// `.pem` is, and what `openssl pkey -pubout` derives from it is the SPKI PEM a member's
/// `.pub` is.
const ALGORITHM: &str = "ed25519";

/// The flags `rola key generate` takes.
#[derive(Pickable)]
struct GenerateFlags {
    /// Put the pair in the user's key directory.
    #[arg(long)]
    install: Flag,
}

#[help(buffer)]
pub fn help_key_generate(_: EntryKeyGenerate, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("key_generate.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryKeyGenerate)]
pub fn desc_key_generate() -> Description {
    t!("key_generate.cmd_key_generate_description")
        .to_string()
        .into()
}

/// Generates an Ed25519 key pair with the system's `openssl`.
///
/// Two files are written, named after the same stem: the private key in PKCS#8 PEM, which is
/// what an account is, and the public key derived from it in SPKI PEM, which is what a member
/// is. They go in the current directory as `<NAME>.pem` and `<NAME>.pub`.
///
/// `NAME` names the *pair*, not one file: an extension on it separates the stem from what a
/// key is written as, so `alice`, `alice.pem` and `alice.pub` all name the same two files.
///
/// With `--install`, the pair goes into the user's key directory under the local data
/// directory instead, and that directory is created if it is not there yet.
///
/// # Errors
///
/// Renders [`ErrorKeyNameMissing`] when `NAME` is not a name, [`ErrorNoOpenSsl`] when
/// `openssl` cannot be run, [`ErrorKeyGenFailed`] when it runs and does not produce both keys,
/// [`ErrorNoKeyDir`] when `--install` cannot find the user's key directory, and
/// [`ErrorInstallDir`] when it cannot create it.
#[command(node = "key.generate")]
pub fn key_generate(args: EntryKeyGenerate) -> Next {
    let picked = args
        .pick(&arg![GenerateFlags])
        .pick_or_route(&arg![String], || ErrorKeyNameMissing.into())
        .to_result();
    let (flags, named) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    // A pair is named by the stem of what was given, which is also what separates a name from
    // what a key is written as: it is the same reading here as everywhere a key is named.
    // What has no stem is not a name, so there is nothing to make a pair under.
    let Some(name) = stem_of(Path::new(&named)) else {
        return ErrorKeyNameMissing.into();
    };

    // Picking a `Flag` cannot fail, so this is `Active` only when it was written.
    StateKeyGenerate {
        name,
        install: matches!(flags.install, Flag::Active),
    }
    .into()
}

/// The state of generating a key pair.
#[derive(Grouped)]
pub struct StateKeyGenerate {
    /// The name the pair is known by.
    name: String,
    /// Whether the pair goes into the user's key directory.
    install: bool,
}

#[chain]
pub fn handle_key_generate(state: StateKeyGenerate) -> Next {
    let StateKeyGenerate { name, install } = state;

    // Installing is where the directory is the program's own to make, so it does; without it
    // the pair goes in the current directory, which is where the run already is.
    let directory = if install {
        let Some(directory) = user_keys_dir() else {
            return ErrorNoKeyDir.into();
        };

        if let Err(error) = fs::create_dir_all(&directory) {
            return ErrorInstallDir {
                path: directory,
                cause: error.to_string(),
            }
            .into();
        }

        directory
    } else {
        PathBuf::new()
    };

    let stem = directory.join(name);
    let private = stem.with_extension(PRIVATE_KEY_EXTENSION);
    let public = stem.with_extension(PUBLIC_KEY_EXTENSION);

    match generate(&private, &public) {
        Ok(()) => ResultKeyGenerated { private, public }.into(),
        Err(error) => error,
    }
}

/// The name a path carries, without the part that says what a key is written as.
fn stem_of(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;

    (!stem.is_empty()).then(|| stem.to_string())
}

/// Asks `openssl` to write both halves of the pair.
///
/// # Errors
///
/// Returns [`ErrorNoOpenSsl`] when `openssl` cannot be run, and [`ErrorKeyGenFailed`] when
/// it runs and one of the two keys is not produced. A public half that fails leaves the
/// private one behind, since neither file is removed on the way out.
fn generate(private: &Path, public: &Path) -> Result<(), Next> {
    // The private half: a PKCS#8 PEM, which is what an account's `.pem` is.
    let written = Command::new("openssl")
        .args(["genpkey", "-algorithm", ALGORITHM, "-out"])
        .arg(private)
        .status();
    match written {
        // `openssl` is not installed, or cannot be run at all.
        Err(_) => return Err(ErrorNoOpenSsl.into()),
        Ok(status) if !status.success() => return Err(ErrorKeyGenFailed.into()),
        Ok(_) => {}
    }

    // The public half: what names the same identity, in SPKI PEM.
    let derived = Command::new("openssl")
        .args(["pkey", "-in"])
        .arg(private)
        .arg("-pubout")
        .arg("-out")
        .arg(public)
        .status();
    match derived {
        Err(_) => Err(ErrorNoOpenSsl.into()),
        Ok(status) if !status.success() => Err(ErrorKeyGenFailed.into()),
        Ok(_) => Ok(()),
    }
}

/// Result: a key pair was written.
#[derive(Grouped)]
pub struct ResultKeyGenerated {
    /// The private key, in PKCS#8 PEM.
    private: PathBuf,
    /// The public key, in SPKI PEM.
    public: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_key_generated(result: ResultKeyGenerated) {
    r_println!(
        "{}",
        t!(
            "key_generate.success",
            private = result.private.display(),
            public = result.public.display()
        )
        .trim()
    );
}

/// Error: no name was given, or what was given is not a name.
#[derive(Grouped)]
pub struct ErrorKeyNameMissing;

impl Failure for ErrorKeyNameMissing {
    fn name(&self) -> &'static str {
        "error_key_name_missing"
    }

    fn reason(&self) -> String {
        t!("key_generate.err_name_missing").trim().to_string()
    }
}

failure!(ErrorKeyNameMissing);

#[renderer(buffer)]
pub fn render_error_key_name_missing(error: ErrorKeyNameMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("key_generate.err_name_missing_help").trim())
    );
    ec.exit_code = EC_ERR_KEY_ARGUMENT;
}

/// Error: `openssl` could not be run.
#[derive(Grouped)]
pub struct ErrorNoOpenSsl;

impl Failure for ErrorNoOpenSsl {
    fn name(&self) -> &'static str {
        "error_no_open_ssl"
    }

    fn reason(&self) -> String {
        t!("key_generate.err_no_openssl").trim().to_string()
    }
}

failure!(ErrorNoOpenSsl);

#[renderer(buffer)]
pub fn render_error_no_open_ssl(error: ErrorNoOpenSsl, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("key_generate.err_no_openssl_help").trim())
    );
    ec.exit_code = EC_ERR_KEYGEN_NO_OPENSSL;
}

/// Error: `openssl` ran, but did not produce both keys.
#[derive(Grouped)]
pub struct ErrorKeyGenFailed;

impl Failure for ErrorKeyGenFailed {
    fn name(&self) -> &'static str {
        "error_key_gen_failed"
    }

    fn reason(&self) -> String {
        t!("key_generate.err_keygen_failed").trim().to_string()
    }
}

failure!(ErrorKeyGenFailed);

#[renderer(buffer)]
pub fn render_error_key_gen_failed(error: ErrorKeyGenFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("key_generate.err_keygen_failed_help").trim())
    );
    ec.exit_code = EC_ERR_KEYGEN_FAILED;
}

/// Error: `--install` found nowhere to install the pair into.
#[derive(Grouped)]
pub struct ErrorNoKeyDir;

impl Failure for ErrorNoKeyDir {
    fn name(&self) -> &'static str {
        "error_no_key_dir"
    }

    fn reason(&self) -> String {
        t!("key_generate.err_no_key_dir").trim().to_string()
    }
}

failure!(ErrorNoKeyDir);

#[renderer(buffer)]
pub fn render_error_no_key_dir(error: ErrorNoKeyDir, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("key_generate.err_no_key_dir_help").trim())
    );
    ec.exit_code = EC_ERR_KEYGEN_NO_KEY_DIR;
}

/// Error: the directory a pair was to be installed into could not be created.
#[derive(Grouped)]
pub struct ErrorInstallDir {
    /// The directory that could not be created.
    path: PathBuf,
    /// Why it could not be created.
    cause: String,
}

impl Failure for ErrorInstallDir {
    fn name(&self) -> &'static str {
        "error_install_dir"
    }

    fn reason(&self) -> String {
        t!(
            "key_generate.err_install_dir",
            path = self.path.display().to_string(),
            reason = self.cause
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorInstallDir);

#[renderer(buffer)]
pub fn render_error_install_dir(err: ErrorInstallDir, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(err.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("key_generate.err_install_dir_help").trim())
    );
    ec.exit_code = EC_ERR_KEYGEN_INSTALL_FAILED;
}
