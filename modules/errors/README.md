# rorolala-errors

The errors that cross the C ABI, and their conversions to and from the Rust errors they
stand for.

Some errors are foreign — `std::io::Error` and `bincode2::Error` both are — so they cannot
carry `#[lazyffi]`, and no exported signature can name them. Each gets a stand-in here
that *can* be exported, holding what a caller actually reads: the message. The two
convert into each other, so a Rust error becomes one of these at the boundary and can be
turned back where the typed error is needed.

What this costs is the typed payload: what crosses is a message, not an `ErrorKind`, so a
caller that needs to branch on the kind of an `io::Error` has to read the message instead.
That is the trade the boundary makes, and it is made once, here, rather than in every
signature that carries an error.
