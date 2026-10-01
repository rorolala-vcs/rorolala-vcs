//! What every command's completion shares: which word of the line is being filled, and how the
//! candidates a run already knows are put to the shell.
//!
//! A completion's answers are the run's own — a bound Vault's name, an account, a path, a hash —
//! so very little is common to them. What is common is reading the command line: a command is
//! reached by a node, the words after it are its positional arguments, and the word in progress
//! is one of them. That counting, and the narrowing of a known list to what the word could still
//! become, are here.
//!
//! Nothing here decides what to offer for a command: that is the command's own file, beside the
//! command itself.

use librorolala::layout::Layout;
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::{VCSIndex, VCSIndexObject};
use mingling::{
    ShellContext, Suggest,
    picker::{
        Pickable, PickerArg, PickerArgInfo,
        parselib::{ParserStyle, build_possible_flags},
    },
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResVault, ResWorkspace};
use rorolala_utils_constants::{VAULT_LAYOUT_NAME, WORKSPACE_READONLY_LAYOUTS_DIR};
use rorolala_utils_location::Locate as _;

use crate::layout::{chosen, readonly_layout_dir};

/// Which positional argument the word being completed is, counting from zero.
///
/// `anchor` is the last word of the node the command is reached by — `bind` for `vault bind` —
/// since that is where the command's own arguments begin. A word already typed fills its
/// position; the word being completed fills the position it is about to, so it is the words
/// after the node that are counted, less the one in progress. A run that names the anchor
/// nowhere counts from the beginning of the line, which is what a top-level command gets.
#[must_use]
pub fn positional(ctx: &ShellContext, anchor: &str) -> usize {
    let after_node = ctx
        .all_words
        .iter()
        .position(|word| word == anchor)
        .map_or(0, |index| index + 1);

    let typed = ctx.all_words.len().saturating_sub(after_node);

    typed.saturating_sub(usize::from(!ctx.current_word.is_empty()))
}

/// Whether the word being completed is a flag being written.
///
/// A word that starts with `-` is the run naming a flag rather than filling an argument, so a
/// completion answers it with the flags the command takes. The global flags are consumed before
/// any command runs and are not a command's to offer.
#[must_use]
pub fn typing_flag(ctx: &ShellContext) -> bool {
    ctx.current_word.starts_with('-')
}

/// Whether `word` is `arg` written as a flag, with a value after the style's separator.
///
/// The flag strings are built from the same `PickerArg` the parse reads — through the parser's own
/// [`build_possible_flags`] — so a name is written once and the two ends cannot come to disagree
/// about it. A flag with a short name or aliases is answered for in every form it may be written.
#[must_use]
pub fn is_flag<T>(word: &str, arg: &'static PickerArg<'static, T>) -> bool
where
    T: Pickable<'static>,
{
    let info = PickerArgInfo::from(arg);
    let style = ParserStyle::global_style();

    build_possible_flags(style, &info).iter().any(|flag| {
        word == flag
            || word
                .strip_prefix(flag.as_str())
                .is_some_and(|value| value.starts_with(style.value_separator))
    })
}

/// Whether the word being completed is the value of `arg`, written after its flag.
#[must_use]
pub fn filling_flag<T>(ctx: &ShellContext, arg: &'static PickerArg<'static, T>) -> bool
where
    T: Pickable<'static>,
{
    is_flag(&ctx.previous_word, arg)
}

/// The positional word written at `index` after the command node, when one is there.
///
/// It is what a command that reads one of its words as a choice needs: which Vault was named, or
/// which Layout, so that the words after it can be answered for.
#[must_use]
pub fn positional_word(ctx: &ShellContext, anchor: &str, index: usize) -> Option<String> {
    let after_node = ctx
        .all_words
        .iter()
        .position(|word| word == anchor)
        .map_or(0, |index| index + 1);

    ctx.all_words.get(after_node + index).cloned()
}

/// The value written after `arg` on the line, when one is there.
///
/// A flag may be named before the argument whose completion needs it — `layout entries --uuid`
/// after `--layout` — so the value already written is read back from the line rather than
/// required to be the word in progress. The value is told apart the way the parser tells it: the
/// flag is built from the same `PickerArg`, and it carries its value either as the next word or
/// after the style's separator.
#[must_use]
pub fn flag_value<T>(ctx: &ShellContext, arg: &'static PickerArg<'static, T>) -> Option<String>
where
    T: Pickable<'static>,
{
    let info = PickerArgInfo::from(arg);
    let style = ParserStyle::global_style();
    let flags = build_possible_flags(style, &info);

    for (index, word) in ctx.all_words.iter().enumerate() {
        if flags.iter().any(|flag| flag == word) {
            return ctx.all_words.get(index + 1).cloned();
        }

        // A value written with its flag is one word, so the flag is the head of it.
        for flag in &flags {
            if let Some(value) = word.strip_prefix(flag.as_str())
                && let Some(value) = value.strip_prefix(style.value_separator)
            {
                return Some(value.to_owned());
            }
        }
    }

    None
}

/// The candidates among `names` that the word in progress could still become.
///
/// An empty answer is `Suggest::Suggest` with nothing in it, not a fall back to the filesystem:
/// what a command takes is the command's to know, and a name that matches nothing is a name
/// that matches nothing.
#[must_use]
pub fn offer<I, S>(ctx: &ShellContext, names: I) -> Suggest
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    names
        .into_iter()
        .filter(|name| name.as_ref().starts_with(&ctx.current_word))
        .map(|name| name.as_ref().to_owned())
        .collect::<Vec<_>>()
        .into()
}

/// Drops the suggestions already written on the line.
///
/// A flag that is already there is not offered again, which is what keeps a long line from
/// growing the same word twice. [`Suggest::FileCompletion`] is handed back untouched: it names
/// no words of its own, so there is none to have been written.
#[must_use]
pub fn strip_written(ctx: &ShellContext, suggest: Suggest) -> Suggest {
    let Suggest::Suggest(mut items) = suggest else {
        return suggest;
    };

    let written: Vec<&str> = ctx.all_words.iter().map(String::as_str).collect();
    items.retain(|item| !written.contains(&item.suggest().as_str()));

    Suggest::Suggest(items)
}

/// The Vault names the Workspace has bound, in name order.
///
/// It is the same set the commands that reach for a Vault resolve, so a name that can be
/// completed is one that would be accepted. A run outside a Workspace has none.
#[must_use]
pub fn vault_names(remote: &ResCurrentRemoteVault) -> Vec<String> {
    let Ok(names) = remote.names() else {
        return Vec::new();
    };

    let mut names: Vec<String> = names.into_iter().map(str::to_string).collect();
    names.sort();

    names
}

/// The Layouts the Workspace itself holds, in name order.
///
/// A fetched copy is not among them: it is read rather than worked in, so a command that changes a
/// Layout names one of these.
#[must_use]
pub fn workspace_layout_names(workspace: &ResWorkspace) -> Vec<String> {
    let Some(workspace) = workspace.as_ref() else {
        return Vec::new();
    };

    workspace.layouts().names().unwrap_or_default()
}

/// The Layouts a run can name, in name order.
///
/// A Workspace's own Layouts are named directly; a Vault's Layout, once fetched, is named
/// `NAME@VAULT`, which is how a query reaches the read-only copy. A Vault's Layout is known by
/// one name, so it is that one that stands before the `@`.
#[must_use]
pub fn layout_names(workspace: &ResWorkspace) -> Vec<String> {
    let Some(held) = workspace.as_ref() else {
        return Vec::new();
    };

    let mut names = workspace_layout_names(workspace);

    // A fetched copy is what makes `NAME@VAULT` a name that resolves, so it is offered only
    // when the copy is there; the cache is a directory per Vault under the Workspace's root.
    if let Ok(vaults) = std::fs::read_dir(held.get_root().join(WORKSPACE_READONLY_LAYOUTS_DIR)) {
        for vault in vaults.flatten() {
            let Some(name) = vault.file_name().to_str().map(str::to_owned) else {
                continue;
            };

            if readonly_layout_dir(held, &name, VAULT_LAYOUT_NAME).is_dir() {
                names.push(format!("{VAULT_LAYOUT_NAME}@{name}"));
            }
        }
    }

    names.sort();
    names.dedup();

    names
}

/// The Layout a run would work on, opened for reading, or nothing when there is none.
///
/// It is [`chosen`] with its failures dropped: a completion has no caller to report to, and a
/// Layout that cannot be opened is one with no content to offer.
#[must_use]
pub fn chosen_layout(
    workspace: &ResWorkspace,
    vault: &ResVault,
    named: Option<&str>,
) -> Option<Layout> {
    chosen(workspace, vault, named).ok()
}

/// The paths the Layout names, in the Layout's own order.
#[must_use]
pub fn layout_paths(layout: &Layout) -> Vec<String> {
    layout
        .paths()
        .into_iter()
        .map(|(path, _)| path.as_str().to_owned())
        .collect()
}

/// The `Uuid`s the Layout holds, in the Layout's own order.
#[must_use]
pub fn layout_uuids(layout: &Layout) -> Vec<String> {
    layout
        .entries()
        .into_iter()
        .map(|(id, _)| id.to_string())
        .collect()
}

/// Which of the index's objects a completion is after.
///
/// A hash names an object, and which object it is is what says whether a command can use it: a
/// version where a version is wanted, a creator where a creator is. The kinds stand as a choice
/// rather than a filter so a call site does not name the index's own enum to say one.
#[derive(Clone, Copy)]
pub enum IndexObject {
    /// Any object the index holds.
    Any,
    /// A Variant.
    Variant,
    /// A Version.
    Version,
    /// A Creator.
    Creator,
    /// A Message.
    Message,
}

impl IndexObject {
    /// Whether `object` is of this kind.
    fn holds(self, object: &VCSIndexObject) -> bool {
        match self {
            Self::Any => true,
            Self::Variant => matches!(object, VCSIndexObject::Variant(_)),
            Self::Version => matches!(object, VCSIndexObject::Version(_)),
            Self::Creator => matches!(object, VCSIndexObject::Creator(_)),
            Self::Message => matches!(object, VCSIndexObject::Message(_)),
        }
    }
}

/// The `Uuid`s of the fetched copy of a Vault's Layout, when one is here to read.
///
/// An ownership exchange names entries of the Vault's own Layout, and what a run has of it is the
/// read-only copy a `layout fetch` left. `named` is the Vault written on the line, or nothing when
/// the Workspace's own choice is reached for.
#[must_use]
pub fn cached_layout_uuids(
    workspace: &ResWorkspace,
    remote: &ResCurrentRemoteVault,
    named: &str,
) -> Vec<String> {
    let Some(workspace) = workspace.as_ref() else {
        return Vec::new();
    };

    let Ok(vault) = remote.name_or_default(named) else {
        return Vec::new();
    };
    let Ok(layout) = Layout::open(readonly_layout_dir(workspace, &vault, VAULT_LAYOUT_NAME)) else {
        return Vec::new();
    };

    layout_uuids(&layout)
}

/// The hashes the index holds of one kind, in the index's own order.
///
/// The index is asynchronous and a completion is not, so the two meet here: the objects are read
/// whole and filtered, which is the same read the `ls-*` commands make. An index that is not
/// there, or will not read, leaves nothing to offer.
#[must_use]
pub fn index_hashes(index: Option<&VCSIndex>, kind: IndexObject) -> Vec<String> {
    let Some(index) = index else {
        return Vec::new();
    };

    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return Vec::new();
    };
    let Ok(objects) = runtime.block_on(index.read_objects()) else {
        return Vec::new();
    };

    objects
        .into_iter()
        .filter(|(_, object)| kind.holds(object))
        .map(|(key, _)| key.hex())
        .collect()
}

/// The keys the store holds.
///
/// It is the listing [`storage ls-storaged`](crate::storage::cmd_storage_ls_storaged) prints, so
/// a key that can be completed is one the store would answer for.
#[must_use]
pub fn store_keys(store: Option<&RorolalaStorage>) -> Vec<String> {
    let Some(store) = store else {
        return Vec::new();
    };

    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return Vec::new();
    };
    let Ok(keys) = runtime.block_on(store.list_exist_keys()) else {
        return Vec::new();
    };

    keys.iter().map(Key::hex).collect()
}
