use std::path::{Path, PathBuf};
use std::process::Command;

use mingling::{
    Grouped,
    macros::{arg, buffer, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::{PRIVATE_KEY_EXTENSION, PUBLIC_KEY_EXTENSION};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_TOOL_KEYGEN_FAILED, EC_ERR_TOOL_KEYGEN_NO_OPENSSL, EC_ERR_TOOL_KEYGEN_PATH_NOT_EXIST,
    EC_HELP,
};

/// The stem a key pair is named by when no path is given.
const KEY_STEM: &str = "key";

/// The algorithm a key pair is generated for.
///
/// Ed25519 is the only one this build knows, and the one the rest of Rorolala reads: what
/// `openssl genpkey` writes for it is a PKCS#8 PEM, which is exactly what an account's
/// `.pem` is, and what `openssl pkey -pubout` derives from it is the SPKI PEM a member's
/// `.pub` is.
const ALGORITHM: &str = "ed25519";

#[help(buffer)]
pub fn help_tool_keygen(_: EntryToolKeygen, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool_keygen.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryToolKeygen)]
pub fn desc_tool_keygen() -> Description {
    t!("tool_keygen.cmd_tool_keygen_description")
        .to_string()
        .into()
}

/// Generates an identity key pair with the system's `openssl`.
///
/// Two files are written, named after the same stem: a private key in PKCS#8 PEM, which is
/// what an account is, and the public key derived from it in SPKI PEM, which is what a
/// member is. The pair goes in the current directory as `key.pem` and `key.pub`, or where
/// the path the caller names says.
///
/// The path names the *pair*, not one file: an extension on it separates the stem from what
/// a key is written as, so `alice`, `alice.pem` and `alice.pub` all name the same two files
/// — `alice.pem` and `alice.pub`. A path that is already a directory holds the pair under
/// the default stem.
///
/// # Errors
///
/// Renders [`ErrorNoOpenSsl`] when `openssl` cannot be run, [`ErrorKeyGenFailed`] when it
/// runs and does not produce both keys, and [`ErrorPathNotExist`] when the path named is
/// not inside a directory that exists.
#[command(node = "tool-keygen")]
pub fn tool_keygen(args: EntryToolKeygen) -> Next {
    // Picking an `Option` cannot fail — an argument that is absent is `None` rather than an
    // error — so this unwrap never panics.
    let (private, public) = pair(args.pick(&arg![Option<PathBuf>]).unwrap());

    // A key pair is written into a directory that has to be there already: naming a path is
    // not a request to create one. The two files share a directory, so one check covers both.
    if let Some(directory) = private.parent()
        && !directory.as_os_str().is_empty()
        && !directory.exists()
    {
        return ErrorPathNotExist {
            path: directory.to_path_buf(),
        }
        .into();
    }

    match generate(&private, &public) {
        Ok(()) => ResultKeyGenerated { private, public }.into(),
        Err(error) => error,
    }
}

/// The private and public files a key pair is written to.
fn pair(output: Option<PathBuf>) -> (PathBuf, PathBuf) {
    let stem = match output {
        None => PathBuf::from(KEY_STEM),
        Some(path) if path.is_dir() => path.join(KEY_STEM),
        Some(path) => path.with_extension(""),
    };

    (
        stem.with_extension(PRIVATE_KEY_EXTENSION),
        stem.with_extension(PUBLIC_KEY_EXTENSION),
    )
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
            "tool_keygen.success",
            private = result.private.display(),
            public = result.public.display()
        )
        .trim()
    );
}

/// Error: `openssl` could not be run.
#[derive(Grouped)]
pub struct ErrorNoOpenSsl;

#[renderer(buffer)]
pub fn render_error_no_open_ssl(_: ErrorNoOpenSsl, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tool_keygen.err_no_openssl").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_keygen.err_no_openssl_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_KEYGEN_NO_OPENSSL;
}

/// Error: `openssl` ran, but did not produce both keys.
#[derive(Grouped)]
pub struct ErrorKeyGenFailed;

#[renderer(buffer)]
pub fn render_error_key_gen_failed(_: ErrorKeyGenFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tool_keygen.err_keygen_failed").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_keygen.err_keygen_failed_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_KEYGEN_FAILED;
}

/// Error: the path named is not inside a directory that exists.
#[derive(Grouped)]
pub struct ErrorPathNotExist {
    /// The directory that was not found.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_error_path_not_exist(err: ErrorPathNotExist, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!(
            "tool_keygen.err_path_not_exist",
            path = err.path.display()
        ))
        .trim()
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_keygen.err_path_not_exist_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_KEYGEN_PATH_NOT_EXIST;
}
