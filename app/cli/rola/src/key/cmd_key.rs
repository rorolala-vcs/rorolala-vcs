//! The `rola key` namespace: the keys Rorolala can reach, and making one.
//!
//! A key is what an account is and what a member is: the private half is the identity the work
//! acts as, and the public half is the name others know it by. Listing them and completing a
//! name are the same list, read from the same place: what is printed is what can be completed.
//!
//! Making a pair is the other half of the namespace, [`key generate`](crate::key::cmd_key_generate),
//! because both are about the same thing — the pair an account is made of.

use std::path::PathBuf;

use librorolala::auth::{locate_accounts, locate_members};
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, empty_result, help, metadata, r_eprintln,
        r_println, renderer, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::keys::{roots, scopes};

/// The flags `rola key` takes.
#[derive(Pickable)]
struct KeyFlags {
    /// Look for private keys — the accounts the work acts as — instead of public ones.
    #[arg(long)]
    pem: Flag,
}

#[help(buffer)]
pub fn help_key(_: EntryKey, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("key.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryKey)]
pub fn desc_key() -> Description {
    t!("key.cmd_key_description").to_string().into()
}

/// Lists the keys Rorolala can reach.
///
/// Every scope is searched, highest priority first: the keys beside the work at hand first —
/// the Workspace's, then the Vault's — and after them the user's own stores, under the local
/// data directory, the filesystem root and `ROLA_HOME`. A key found higher up shadows the
/// same name below it, so that is the order a name would resolve in. By default the keys
/// listed are public ones, the members a name can mean; with `--pem` they are private ones,
/// the accounts the work can act as.
///
/// Each key found is printed as a full path, one per line, on standard output, in the order
/// it is looked up. Finding nothing is not a failure: it prints nothing and returns.
#[command(node = "key", entry = EntryKey)]
pub fn key(args: EntryKey) -> StateKeyList {
    // Picking flags cannot fail: a flag that is absent is `Inactive`, not an error.
    let flags = args.pick(&arg![KeyFlags]).unwrap();

    StateKeyList::from(matches!(flags.pem, Flag::Active))
}

/// The state a listing of keys starts in.
///
/// Which halves of the pairs are listed is the whole of what a listing is told, so this is
/// the one thing said about it before there is anything to look for.
#[derive(Grouped, Clone, Copy, Wrap)]
pub struct StateKeyList {
    /// Whether the keys listed are the private ones, the accounts the work can act as.
    pem: bool,
}

#[chain]
pub fn handle_key_list(
    state: StateKeyList,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Next {
    let StateKeyList { pem } = state;
    let roots = roots(workspace.get_ref().as_ref(), vault.get_ref().as_ref());
    let scopes = scopes();

    let paths: Vec<PathBuf> = if pem {
        locate_accounts(&roots, &scopes)
            .into_iter()
            .map(|key| key.key_path())
            .collect()
    } else {
        locate_members(&roots, &scopes)
            .into_iter()
            .map(|key| key.key_path())
            .collect()
    };

    if paths.is_empty() {
        // A place with no keys is not something to complain about.
        empty_result!()
    } else {
        ResultKeysFound { paths }.into()
    }
}

/// Completes what `rola key` can be given next.
///
/// The command takes no positional word of its own and its subcommands are offered by the
/// dispatcher, so only a word that starts a flag is answered at all — and only with the flags
/// that are not already on the line.
#[completion(EntryKey)]
pub fn complete_key(ctx: ShellContext) -> Suggest {
    if !ctx.current_word.starts_with('-') {
        return suggest!();
    }

    let typed: Vec<&str> = ctx.all_words.iter().map(String::as_str).collect();
    let mut suggestions = suggest! {
        "--pem": t!("key.complete.pem"),
    };

    if let Suggest::Suggest(items) = &mut suggestions {
        items.retain(|item| !typed.contains(&item.suggest().as_str()));
    }

    suggestions
}

/// Result: keys were found beside the work at hand.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultKeysFound {
    /// The full path of each key, in the order `locate` ranked them.
    paths: Vec<PathBuf>,
}

#[renderer(buffer)]
pub fn render_result_keys_found(result: ResultKeysFound) {
    for path in result.paths {
        r_println!("{}", path.display());
    }
}
