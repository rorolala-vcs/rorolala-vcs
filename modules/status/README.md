# rorolala-status

How the work stands: the tree beside the Layout it works from, who holds what, and the merges in
progress.

It is the reading behind `rola status` with nothing named, and behind the Desktop's Overview, and it
reads rather than works: the tree analysis, the fetched copy of the Vault's Layout and the merge
records under the Workspace's own data directory are all things that already exist, and what is
added here is the one answer that says how they stand together — see [`Status`].

The chain of one file is read here too, for `rola status <file>` and the Desktop's Version Graph:
what it is made of is the walk [`read_chain`](rorolala_vcs::read_chain) makes. The command line lays
the chain out as its own character grid; the Desktop takes the versions, variants and joins across
the ABI and draws its own geometry.

What a reader does about what it finds is not this crate's to say: a lost path is settled by
`rola align` and a changed one by `rola track`, and the words for both belong to the program that
shows them. So what crosses is the reading itself — paths, the moves among them, the variants
waiting to be joined — and never a sentence.
