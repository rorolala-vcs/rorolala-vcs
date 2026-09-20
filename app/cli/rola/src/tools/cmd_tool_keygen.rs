use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use librorolala::auth::user_keys_dir;
use mingling::{
    Grouped,
    macros::{arg, buffer, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::{PRIVATE_KEY_EXTENSION, PUBLIC_KEY_EXTENSION};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_TOOL_KEYGEN_FAILED, EC_ERR_TOOL_KEYGEN_INSTALL_FAILED, EC_ERR_TOOL_KEYGEN_NO_KEY_DIR,
    EC_ERR_TOOL_KEYGEN_NO_OPENSSL, EC_ERR_TOOL_KEYGEN_PATH_NOT_EXIST, EC_HELP,
};

/// The stem a key pair is named by when neither a name nor a path says otherwise.
const KEY_STEM: &str = "key";

/// The flags `rola tool-keygen` takes.
#[derive(Pickable)]
struct KeygenFlags {
    /// The name the pair is known by, instead of `key`.
    #[arg(long)]
    name: Option<String>,
    /// Install the pair into the user's key directory.
    #[arg(long)]
    install: Flag,
}

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
/// member is. The pair goes in the current directory as `key.pem` and `key.pub`, under the
/// name `--name` gives, or where the path the caller names says.
///
/// The path names the *pair*, not one file: an extension on it — or on `--name` — separates
/// the stem from what a key is written as, so `alice`, `alice.pem` and `alice.pub` all name
/// the same two files. A path that is already a directory holds the pair under `--name`, or
/// under the default stem.
///
/// With `--install`, the pair goes into the user's key directory under the local data
/// directory instead, and that directory is created if it is not there yet.
///
/// # Errors
///
/// Renders [`ErrorNoOpenSsl`] when `openssl` cannot be run, [`ErrorKeyGenFailed`] when it
/// runs and does not produce both keys, [`ErrorPathNotExist`] when the path named is not
/// inside a directory that exists, [`ErrorNoKeyDir`] when `--install` cannot find the
/// user's key directory, and [`ErrorInstallDir`] when it cannot create it.
#[command(node = "tool-keygen")]
pub fn tool_keygen(args: EntryToolKeygen) -> Next {
    // Picking cannot fail — a flag that is absent is `Inactive`, and an option or a
    // positional that is absent is `None` — so this unwrap never panics.
    let (flags, output) = args
        .pick(&arg![KeygenFlags])
        .pick(&arg![Option<PathBuf>])
        .unwrap();

    let install = matches!(flags.install, Flag::Active);

    let Some((directory, name)) = destination(output.as_deref(), flags.name.as_deref(), install)
    else {
        return ErrorNoKeyDir.into();
    };

    // A key pair is written into a directory that has to be there already: naming a path is
    // not a request to create one. Installing is the exception — the directory is the
    // program's own to make, so it does.
    if install {
        if let Err(error) = fs::create_dir_all(&directory) {
            return ErrorInstallDir {
                path: directory,
                reason: error.to_string(),
            }
            .into();
        }
    } else if !directory.as_os_str().is_empty() && !directory.exists() {
        return ErrorPathNotExist { path: directory }.into();
    }

    let stem = directory.join(name);
    let private = stem.with_extension(PRIVATE_KEY_EXTENSION);
    let public = stem.with_extension(PUBLIC_KEY_EXTENSION);

    match generate(&private, &public) {
        Ok(()) => ResultKeyGenerated { private, public }.into(),
        Err(error) => error,
    }
}

/// Where a pair is written: the directory it goes into and the name it is known by.
///
/// The name is the one `--name` states, or the name the path carries, or `key`. The
/// directory is the user's key directory when `--install` says so, the one the path names,
/// or the current directory. `None` means `--install` was asked for on a machine that does
/// not say where the user's local data directory is, so there is nowhere to install to.
fn destination(
    output: Option<&Path>,
    name: Option<&str>,
    install: bool,
) -> Option<(PathBuf, String)> {
    let directory = if install {
        user_keys_dir()?
    } else {
        match output {
            None => PathBuf::new(),
            Some(path) if path.is_dir() => path.to_path_buf(),
            Some(path) => path.parent().map_or_else(PathBuf::new, Path::to_path_buf),
        }
    };

    let name = name
        .and_then(|name| stem_of(Path::new(name)))
        .or_else(|| output.filter(|path| !path.is_dir()).and_then(stem_of))
        .unwrap_or_else(|| KEY_STEM.to_string());

    Some((directory, name))
}

/// The name a path carries, without the part that says what a key is written as.
fn stem_of(path: &Path) -> Option<String> {
    Some(path.file_stem()?.to_str()?.to_string())
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

/// Error: `--install` found nowhere to install the pair into.
#[derive(Grouped)]
pub struct ErrorNoKeyDir;

#[renderer(buffer)]
pub fn render_error_no_key_dir(_: ErrorNoKeyDir, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tool_keygen.err_no_key_dir").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_keygen.err_no_key_dir_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_KEYGEN_NO_KEY_DIR;
}

/// Error: the directory a pair was to be installed into could not be created.
#[derive(Grouped)]
pub struct ErrorInstallDir {
    /// The directory that could not be created.
    path: PathBuf,
    /// Why it could not be created.
    reason: String,
}

#[renderer(buffer)]
pub fn render_error_install_dir(err: ErrorInstallDir, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(
            t!(
                "tool_keygen.err_install_dir",
                path = err.path.display().to_string(),
                reason = err.reason
            )
            .trim()
        )
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_keygen.err_install_dir_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_KEYGEN_INSTALL_FAILED;
}
