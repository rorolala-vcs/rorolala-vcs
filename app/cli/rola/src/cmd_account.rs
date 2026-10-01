//! The `rola account` command: which accounts the work can act as.
//!
//! An account is a private key, and the one the work acts as is a choice of the user's own
//! rather than of any one Vault or Workspace, so it is kept in Rorolala's own file for the
//! user. Listing the accounts and completing a name are the same list, read from the same
//! place: what is printed is what can be completed.

use std::path::PathBuf;

use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, empty_result, help, metadata, r_eprintln, r_print,
        r_println, renderer, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::{EC_ERR_ACCOUNT_NO_DIR, EC_ERR_ACCOUNT_NOT_FOUND, EC_ERR_FORMAT, EC_HELP};
use crate::failure::failure;
use crate::format::ResFormat;
use crate::keys::account_names;
use crate::user::account_path;

/// How a listing of the accounts is drawn when no template is named.
///
/// One name a line, which is what the command has always printed. Each account also carries
/// whether it is the one the work acts as, `is_current`, in the data a template or `--json`
/// reads.
const DEFAULT_ACCOUNT_LS_FORMAT: &str = "{{ accounts.name }}";

/// How the current account is drawn when no template is named.
const DEFAULT_ACCOUNT_CURRENT_FORMAT: &str = "{{ name }}";

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
pub fn account(args: EntryAccount) -> Next {
    // Picking an `Option` cannot fail: an account that is absent is `None` rather than an
    // error, so this unwrap never panics.
    let chosen: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    let Some(name) = chosen else {
        return StateAccountList.into();
    };

    StateAccountSet::from(name).into()
}

/// The state a listing of the accounts starts in.
///
/// Listing names nothing, so there is nothing to say about it beyond the fact that it was
/// asked for: what it lists is whatever any scope holds.
#[derive(Grouped)]
pub struct StateAccountList;

#[chain(routeify)]
pub fn handle_account_list(
    _state: StateAccountList,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let names = account_names(workspace.get_ref().as_ref(), vault.get_ref().as_ref());

    ResultAccounts {
        current: current.get_ref().name().map(str::to_string),
        names,
    }
    .into()
}

/// The state of naming the account the work acts as.
#[derive(Grouped, Wrap)]
pub struct StateAccountSet {
    /// The account that was named.
    name: String,
}

#[chain(routeify)]
pub fn handle_account_set(
    state: StateAccountSet,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let StateAccountSet { name } = state;
    let names = account_names(workspace.get_ref().as_ref(), vault.get_ref().as_ref());

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

#[help(buffer)]
pub fn help_account_ls(_: EntryAccountLs, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("account_ls.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryAccountLs)]
pub fn desc_account_ls() -> Description {
    t!("account_ls.description").to_string().into()
}

/// Completes what `rola account ls` can be given next.
///
/// The listing names nothing, so there is nothing to offer.
#[completion(EntryAccountLs)]
pub fn complete_account_ls() -> Suggest {
    suggest!()
}

/// Lists the accounts the work can act as, each with whether it is the one
///
/// Every account any scope holds, in name order, drawn through a template the way every query is:
/// the default names them one a line, and each carries `is_current` — whether it is the account the
/// work acts as — so a run that reads the output as a program can tell which one is chosen without
/// a marker a person would read.
#[command(node = "account.ls")]
pub fn account_ls(format: &mut ResFormat) -> StateAccountLs {
    format.default_template(DEFAULT_ACCOUNT_LS_FORMAT);
    StateAccountLs
}

/// The state a listing of the accounts starts in.
#[derive(Grouped)]
pub struct StateAccountLs;

#[chain(routeify)]
pub fn handle_account_ls(
    _state: StateAccountLs,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
    current: &mut LazyRes<ResCurrentAccount>,
    format: &mut ResFormat,
) -> Next {
    let names = account_names(workspace.get_ref().as_ref(), vault.get_ref().as_ref());
    let current = current.get_ref().name().map(str::to_string);

    let accounts: Vec<AccountItem> = names
        .into_iter()
        .map(|name| AccountItem {
            is_current: current.as_deref() == Some(name.as_str()),
            name,
        })
        .collect();

    format.set(
        "accounts",
        accounts
            .iter()
            .map(|account| serde_json::json!(account))
            .collect(),
    );

    ResultAccountLs { accounts }.into()
}

/// One account, as `account ls` shows it.
#[derive(Serialize)]
pub struct AccountItem {
    /// The account's name.
    name: String,
    /// Whether it is the account the work acts as.
    is_current: bool,
}

/// Result: the accounts the work can act as were listed, each with whether it is the one.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultAccountLs {
    /// The accounts, in name order.
    accounts: Vec<AccountItem>,
}

#[renderer(buffer)]
pub fn render_result_account_ls(result: ResultAccountLs, format: &ResFormat, ec: &mut ResExitCode) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!(
                    "{}",
                    err_line!(t!("format.err_format", reason = error).trim())
                );
                ec.exit_code = EC_ERR_FORMAT;
            }
        }
    } else {
        for account in &result.accounts {
            r_println!("{}", account.name);
        }
    }
}

#[help(buffer)]
pub fn help_account_current(_: EntryAccountCurrent, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("account_current.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryAccountCurrent)]
pub fn desc_account_current() -> Description {
    t!("account_current.description").to_string().into()
}

/// Completes what `rola account current` can be given next.
///
/// The command names nothing, so there is nothing to offer.
#[completion(EntryAccountCurrent)]
pub fn complete_account_current() -> Suggest {
    suggest!()
}

/// Prints the account the work acts as
///
/// The name alone, so a run can read it without a sentence around it. Nothing is printed when the
/// work acts as no account, since there is none to name.
#[command(node = "account.current")]
pub fn account_current(format: &mut ResFormat) -> StateAccountCurrent {
    format.default_template(DEFAULT_ACCOUNT_CURRENT_FORMAT);
    StateAccountCurrent
}

/// The state printing the current account starts in.
#[derive(Grouped)]
pub struct StateAccountCurrent;

#[chain(routeify)]
pub fn handle_account_current(
    _state: StateAccountCurrent,
    current: &mut LazyRes<ResCurrentAccount>,
    format: &mut ResFormat,
) -> Next {
    let Some(name) = current.get_ref().name() else {
        return empty_result!();
    };

    let name = name.to_string();
    format.set("name", vec![serde_json::json!(name)]);

    ResultAccountCurrent { name }.into()
}

/// Result: the account the work acts as was printed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultAccountCurrent {
    /// The account the work acts as.
    name: String,
}

#[renderer(buffer)]
pub fn render_result_account_current(
    result: ResultAccountCurrent,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!(
                    "{}",
                    err_line!(t!("format.err_format", reason = error).trim())
                );
                ec.exit_code = EC_ERR_FORMAT;
            }
        }
    } else {
        r_println!("{}", result.name);
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

impl Failure for ErrorAccountNotFound {
    fn name(&self) -> &'static str {
        "error_account_not_found"
    }

    fn reason(&self) -> String {
        t!("account.err_not_found", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorAccountNotFound);

#[renderer(buffer)]
pub fn render_error_account_not_found(error: ErrorAccountNotFound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("account.err_not_found_help").trim()));
    ec.exit_code = EC_ERR_ACCOUNT_NOT_FOUND;
}

/// Error: the machine does not name the directory Rorolala keeps the user's files in.
#[derive(Grouped)]
pub struct ErrorNoUserDir;

impl Failure for ErrorNoUserDir {
    fn name(&self) -> &'static str {
        "error_no_user_dir"
    }

    fn reason(&self) -> String {
        t!("account.err_no_user_dir").trim().to_string()
    }
}

failure!(ErrorNoUserDir);

#[renderer(buffer)]
pub fn render_error_no_user_dir(error: ErrorNoUserDir, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("account.err_no_user_dir_help").trim()));
    ec.exit_code = EC_ERR_ACCOUNT_NO_DIR;
}
