//! The backend rola gives jj.
//!
//! jj reaches its storage through one trait — [`jj_lib::backend::Backend`] — and the backend it
//! ships is Git's, which this build does not carry: `jj-lib` and `jj-cli` are depended on with
//! `default-features = false`, so no Git repository is read or written anywhere in this program.
//!
//! What is left is a store that is rola's, and this is its place: a backend named [`NAME`],
//! registered beside jj's own factories so that a repository whose store was written as one rola
//! made is opened by it. Every method is a hole — `todo!()` — which is deliberate and is what is
//! meant by an empty jj: the command line, the argument parsing, the configuration and the
//! templates are jj's and work; what would reach the repository stops where rola's work has not
//! been done.
//!
//! The holes are written by the `rola_backend!` macro below rather than by hand. The trait is jj's,
//! it is long, and each body is the same sentence with the method's own name in it: a method added
//! to the trait upstream should arrive as one line here, and the compiler should be the one that
//! says so.

use std::path::Path;
use std::pin::Pin;

use futures::AsyncRead;
use futures::stream::BoxStream;
use jj_lib::backend::BackendResult;
use jj_lib::backend::ChangeId;
use jj_lib::backend::Commit;
use jj_lib::backend::CommitId;
use jj_lib::backend::CopyHistory;
use jj_lib::backend::CopyId;
use jj_lib::backend::CopyRecord;
use jj_lib::backend::FileId;
use jj_lib::backend::RelatedCopy;
use jj_lib::backend::SigningFn;
use jj_lib::backend::SymlinkId;
use jj_lib::backend::Tree;
use jj_lib::backend::TreeId;
use jj_lib::repo::StoreFactories;
use jj_lib::repo_path::RepoPath;
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::settings::UserSettings;

/// The name rola's backend is known by, which is what a repository's store is written as.
///
/// A store written as one thing is opened by the factory registered under that name, which is why
/// this is rola's own and not jj's: the point of the name is that a repository says whose objects
/// it holds.
pub const NAME: &str = "rola";

/// rola's backend: the place a rola repository's objects will be read and written.
///
/// It carries nothing yet. What it will carry — where the objects are kept, how they are named —
/// is what the methods below are holes for, and a type that held a path now would be one to
/// rewrite rather than fill in.
#[derive(Debug)]
pub struct RolaBackend;

/// What rola adds to jj's own store factories.
///
/// jj's own are what the runner already holds — the in-memory `SimpleBackend` among them, which is
/// what jj carries without Git — and the runner merges this into them, so a store written as
/// [`NAME`] is opened by rola's backend and every other store is still jj's.
///
/// The store's path and the run's settings are what a backend is made over, and neither is read
/// yet: what the backend is made of is what its methods are holes for.
#[must_use]
pub fn store_factories() -> StoreFactories {
    let mut factories = StoreFactories::empty();

    factories.add_backend(
        NAME,
        Box::new(|_settings: &UserSettings, _store_path: &Path| Ok(Box::new(RolaBackend))),
    );

    factories
}

/// Writes [`RolaBackend`]'s implementation, with every body left as a hole.
///
/// Each signature is written here the way jj writes it in the trait, and each body becomes a
/// `todo!()` naming the method that is missing — so filling one in is replacing one line, and a
/// method jj adds is a compile error naming the line that is not there.
///
/// The methods are in two groups because the keyword has to be told apart when the signatures are
/// matched: `async fn` is a repeated fragment's own keyword and a repetition of a bare keyword
/// across a repetition — `$(async)?` — is not something the macro language has. Which methods are
/// asynchronous is jj's business and not this file's: what is written below is what the trait says.
///
/// Every body takes jj's own parameter names and uses none of them, so the unused-parameter lint is
/// excused there rather than the names being thrown away: a hole should still read as the signature
/// it will be filled in from.
macro_rules! rola_backend {
    (
        sync {
            $(
                $(#[$s_meta:meta])*
                fn $s_name:ident($($s_args:tt)*) -> $s_ret:ty;
            )*
        }
        async {
            $(
                $(#[$a_meta:meta])*
                async fn $a_name:ident($($a_args:tt)*) -> $a_ret:ty;
            )*
        }
    ) => {
        #[::async_trait::async_trait]
        impl ::jj_lib::backend::Backend for RolaBackend {
            $(
                $(#[$s_meta])*
                #[allow(
                    unused_variables,
                    reason = "every body here is a hole, so no parameter is used yet"
                )]
                fn $s_name($($s_args)*) -> $s_ret {
                    todo!(concat!("rola's backend does not ", stringify!($s_name), " yet"))
                }
            )*
            $(
                $(#[$a_meta])*
                #[allow(
                    unused_variables,
                    reason = "every body here is a hole, so no parameter is used yet"
                )]
                async fn $a_name($($a_args)*) -> $a_ret {
                    todo!(concat!("rola's backend does not ", stringify!($a_name), " yet"))
                }
            )*
        }
    };
}

rola_backend! {
    sync {
        /// The name this backend is known by, as a repository's store writes it.
        fn name(&self) -> &str;

        /// The length of a commit id in bytes.
        fn commit_id_length(&self) -> usize;

        /// The length of a change id in bytes.
        fn change_id_length(&self) -> usize;

        /// The id of the root commit, which is the ancestor of every commit and has no parents.
        fn root_commit_id(&self) -> &CommitId;

        /// The change id of the root commit.
        fn root_change_id(&self) -> &ChangeId;

        /// The id of the empty tree, which is the same id in every backend.
        fn empty_tree_id(&self) -> &TreeId;

        /// How many requests this backend handles well at once, which is at least one.
        fn concurrency(&self) -> usize;

        /// Streams the copy records between two commits, parents before children.
        fn get_copy_records(
            &self,
            paths: Option<&[RepoPathBuf]>,
            root: &CommitId,
            head: &CommitId,
        ) -> BackendResult<BoxStream<'_, BackendResult<CopyRecord>>>;
    }

    async {
        /// Reads a file's contents.
        async fn read_file(
            &self,
            path: &RepoPath,
            id: &FileId,
        ) -> BackendResult<Pin<Box<dyn AsyncRead + Send>>>;

        /// Writes a file's contents, and answers the id they are kept under.
        async fn write_file(
            &self,
            path: &RepoPath,
            contents: &mut (dyn AsyncRead + Send + Unpin),
        ) -> BackendResult<FileId>;

        /// Reads what a symlink points at, which is not necessarily inside the repository.
        async fn read_symlink(&self, path: &RepoPath, id: &SymlinkId) -> BackendResult<String>;

        /// Writes a symlink, and answers the id it is kept under.
        async fn write_symlink(&self, path: &RepoPath, target: &str) -> BackendResult<SymlinkId>;

        /// Reads what was recorded of where a file's contents came from.
        async fn read_copy(&self, id: &CopyId) -> BackendResult<CopyHistory>;

        /// Writes what is recorded of where a file's contents came from.
        async fn write_copy(&self, copy: &CopyHistory) -> BackendResult<CopyId>;

        /// Finds the copies related to one, children before parents.
        async fn get_related_copies(&self, copy_id: &CopyId) -> BackendResult<Vec<RelatedCopy>>;

        /// Reads a tree.
        async fn read_tree(&self, path: &RepoPath, id: &TreeId) -> BackendResult<Tree>;

        /// Writes a tree, and answers the id it is kept under.
        async fn write_tree(&self, path: &RepoPath, contents: &Tree) -> BackendResult<TreeId>;

        /// Reads a commit.
        async fn read_commit(&self, id: &CommitId) -> BackendResult<Commit>;

        /// Writes a commit, and answers what was written rather than what was handed over.
        ///
        /// The signing function's lifetime is written as the trait writes it — elided — because an
        /// implementation's signature is the trait's signature and not a spelling of its own; the
        /// lint that wants every lifetime in a path spelled out is excused here for that reason.
        #[allow(
            elided_lifetimes_in_paths,
            reason = "an impl signature has to match the trait's, which writes the lifetime elided"
        )]
        async fn write_commit(
            &self,
            contents: Commit,
            sign_with: Option<&mut SigningFn>,
        ) -> BackendResult<(CommitId, Commit)>;
    }
}
