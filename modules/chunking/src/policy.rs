//! What a policy is, and what the registry holds.

use crate::Chunker;

/// A cut a store can be told to use by name.
///
/// A policy is three things and nothing else: a [`Chunker`], the name it is asked for by, and the
/// containers it claims. Everything else a store needs to know — how big the chunks are, what a
/// format's blocks look like — is the policy's own business, kept inside the type that implements
/// this.
///
/// The two consts are consts because the table they feed is a `static`: a name has to be known at
/// compile time for the table to be built at all, which is also what makes a store's refusal of an
/// unknown name a lookup rather than a guess.
///
/// The build stops rather than letting two policies disagree about either one: a name that names two
/// cuts would make a store's answer depend on the order of the table, and a container claimed twice
/// would do the same to a file's.
pub trait Policy: Chunker {
    /// The name a store is told to use this policy by, in kebab-case.
    ///
    /// It is what a command line names, what a store's configuration names, and what a file's
    /// preference is recorded as. It is not written down in anything a store keeps, so changing it
    /// changes nothing already stored.
    const NAME: &'static str;

    /// The container extensions this policy claims, lowercase and without a dot.
    ///
    /// A claim is how a policy is reached without being named: a file whose extension is claimed is
    /// cut by the policy that claims it. An empty list is a policy that claims nothing, which is a
    /// policy reached only by being named — and is what a cut that works on any format gives.
    const EXTENSIONS: &'static [&'static str];
}

/// One row of the registry: a policy as the table can hold it.
///
/// `dyn Chunker` erases the type but not the name, so a name and a set of claims are copied here by
/// the generated table rather than asked of the cut. They are the policy's own consts — the table
/// names `NAME` and `EXTENSIONS`, it does not repeat the literals — so they are written down once.
pub struct PolicyEntry {
    /// The name a store is told to use it by.
    pub name: &'static str,
    /// The containers it claims.
    pub extensions: &'static [&'static str],
    /// What it cuts with.
    pub chunker: &'static dyn Chunker,
}
