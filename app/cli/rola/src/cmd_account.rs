//! The `rola account` command: which accounts the work can act as.
//!
//! An account is a private key, and the one the work acts as is a choice of the user's own
//! rather than of any one Vault or Workspace, so it is kept in Rorolala's own file for the
//! user. Listing the accounts and completing a name are the same list, read from the same
//! place: what is printed is what can be completed.

use std::path::PathBuf;

use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, command, completion, help, metadata, r_eprintln, r_println, renderer, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::{EC_ERR_ACCOUNT_NO_DIR, EC_ERR_ACCOUNT_NOT_FOUND, EC_HELP};
use crate::keys::account_names;
use crate::user::account_path;

#[help(buffer)]
pub fn help_account(_: EntryAccount, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("account.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryAccount)]
pub fn desc_account() -> Description {
    t!("account.cmd_account_description").to_string().into()
}

/// Names the account the work acts as, or lists the accounts it can act as.
///
/// Naming no account lists every account any scope holds — deduplicated and in name order,
/// which is the same list completion offers. Naming one records it as the account the work
/// acts as by default, in Rorolala's own file for the user; a name that is not an account
/// anywhere is reported rather than recorded.
#[command(node = "account")]
pub fn account(
    args: EntryAccount,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let names = account_names(workspace.get_ref().as_ref(), vault.get_ref().as_ref());

    // Picking an `Option` cannot fail: an account that is absent is `None` rather than an
    // error, so this unwrap never panics.
    let chosen: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    let Some(name) = chosen else {
        return ResultAccounts {
            current: current.get_ref().name().map(str::to_string),
            names,
        }
        .into();
    };

    if !names.contains(&name) {
        return ErrorAccountNotFound { name }.into();
    }

    // A machine that does not name where the user's files go has nowhere to record the
    // choice, which is worth saying rather than accepting and losing. What is named is
    // written back as the resource is dropped.
    let Some(path) = account_path() else {
        return ErrorNoUserDir.into();
    };

    current.get_mut().set(name.clone());

    ResultAccountSet { name, path }.into()
}

/// Completes what `rola account` can be given next, from the accounts there are.
///
/// It is the same list the command prints, so a name that can be completed is one that would
/// be accepted.
#[completion(EntryAccount)]
pub fn complete_account(
    ctx: ShellContext,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Suggest {
    if ctx.current_word.starts_with('-') {
        return suggest!();
    }

    let mut names = account_names(workspace.get_ref().as_ref(), vault.get_ref().as_ref());
    names.retain(|name| name.starts_with(&ctx.current_word));

    suggest! { names }
}

/// Result: the accounts the work can act as were listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultAccounts {
    /// Each account name, in name order.
    names: Vec<String>,
    /// The account the work acts as, if one has been named.
    current: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_accounts(result: ResultAccounts) {
    if result.names.is_empty() {
        r_println!("{}", t!("account.result_none").trim());
    } else {
        for name in &result.names {
            if result.current.as_deref() == Some(name.as_str()) {
                r_println!("{}", t!("account.result_current", name = name).trim());
            } else {
                r_println!("{name}");
            }
        }
    }
}

/// Result: an account was named.
#[derive(Grouped)]
pub struct ResultAccountSet {
    /// The name that was recorded.
    name: String,
    /// The file it was recorded in.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_account_set(result: ResultAccountSet) {
    r_println!(
        "{}",
        t!(
            "account.result_set",
            name = result.name,
            path = result.path.display().to_string()
        )
        .trim()
    );
}

/// Error: the name given is not an account the work can act as.
#[derive(Grouped)]
pub struct ErrorAccountNotFound {
    /// The name that is not an account.
    name: String,
}

#[renderer(buffer)]
pub fn render_error_account_not_found(error: ErrorAccountNotFound, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("account.err_not_found", name = error.name).trim())
    );
    r_eprintln!("{}", help_line!(t!("account.err_not_found_help").trim()));
    ec.exit_code = EC_ERR_ACCOUNT_NOT_FOUND;
}

/// Error: the machine does not name the directory Rorolala keeps the user's files in.
#[derive(Grouped)]
pub struct ErrorNoUserDir;

#[renderer(buffer)]
pub fn render_error_no_user_dir(_: ErrorNoUserDir, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("account.err_no_user_dir").trim()));
    r_eprintln!("{}", help_line!(t!("account.err_no_user_dir_help").trim()));
    ec.exit_code = EC_ERR_ACCOUNT_NO_DIR;
}
