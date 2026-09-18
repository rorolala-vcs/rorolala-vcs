use std::path::PathBuf;

use librorolala::auth::{KeyLocateRule, locate_accounts, locate_members};
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, command, completion, empty_result, help, metadata, r_eprintln, r_println,
        renderer, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rorolala_utils_constants::{VAULT_KEYS_DIR, WORKSPACE_KEYS_DIR};
use rorolala_utils_location::Locate;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;

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

/// Lists the keys beside the current Vault or Workspace.
///
/// Both are sniffed from the current directory, and only the directories beside the work at
/// hand are looked in — the Workspace's first, then the Vault's — so what is listed is what
/// this place would resolve a name against. By default the keys listed are public ones, the
/// members a name can mean; with `--pem` they are private ones, the accounts the work can
/// act as.
///
/// Each key found is printed as a full path, one per line, on standard output, in the order
/// it is looked up. Finding nothing is not a failure: it prints nothing and returns.
#[command(node = "key", entry = EntryKey)]
pub fn key(
    args: EntryKey,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Next {
    // The Workspace's keys come first, then the Vault's, which is the order `locate` searches
    // them in: a key kept beside the copy being worked in shadows the same name in the Vault.
    let mut roots = Vec::new();
    if let Some(held) = workspace.get_ref().as_ref() {
        roots.push(held.get_root().join(WORKSPACE_KEYS_DIR));
    }
    if let Some(held) = vault.get_ref().as_ref() {
        roots.push(held.get_root().join(VAULT_KEYS_DIR));
    }

    // Picking flags cannot fail: a flag that is absent is `Inactive`, not an error.
    let flags = args.pick(&arg![KeyFlags]).unwrap();
    let scopes = scopes();

    let paths: Vec<PathBuf> = if matches!(flags.pem, Flag::Active) {
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

/// The scopes a key is looked for in: only the directories beside the work at hand.
///
/// A key in the user's, the machine's, or `ROLA_HOME`'s store belongs to no particular Vault
/// or Workspace, so a listing of what is *here* does not reach it.
const fn scopes() -> KeyLocateRule {
    KeyLocateRule {
        find_global: false,
        find_local: true,
        find_user: false,
        find_env: false,
    }
}

/// Completes what `rola key` can be given next.
///
/// The command has one flag and takes no other word, so only a word that starts a flag is
/// answered at all — and only with the flags that are not already on the line.
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
#[derive(Grouped)]
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
