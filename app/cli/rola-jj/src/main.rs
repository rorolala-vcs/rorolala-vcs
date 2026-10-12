#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use jj_cli::cli_util::CliRunner;

use crate::backend::store_factories;

mod backend;

/// How much stack the command line is given, in bytes.
///
/// jj's command line is deep — clap's tree, the configuration, and the asynchronous futures under
/// both — and at `opt-level = 0` it needs more than the mebibyte a thread is given by default: jj
/// asks its linker for these eight mebibytes in its own `.cargo/config.toml`, for exactly that
/// reason. The same eight are asked for here, and in a place that travels with the program rather
/// than with the build, because a `rola-jj` an export laid down was built somewhere else entirely.
/// The main thread keeps the stack the platform gave it; the command line runs on one of this size.
const STACK: usize = 8 * 1024 * 1024;

/// What a run that panicked ends with, which is what the Rust runtime answers with.
const PANIC: u8 = 101;

/// Runs jj's command line over rola's store.
///
/// The command line is jj-cli's whole: the commands, their arguments and their help are the ones
/// `jj` answers with, so nothing about them is written here. What is this program's is the store
/// they are run over — [`store_factories`] hands the runner rola's backend, which the runner merges
/// into jj's own factories — and the name and version it is drawn as.
///
/// The version is this program's own, and deliberately not jj's: jj's is baked into `jj-cli` at its
/// own build time and is not handed on to a crate that embeds it, and a program that reported the
/// version of a library it happens to carry would be lying about itself. What it carries is in
/// `Cargo.toml`.
fn main() -> std::process::ExitCode {
    let run = std::thread::Builder::new()
        .name("rola jj".to_owned())
        .stack_size(STACK)
        .spawn(|| {
            CliRunner::init()
                .name("rola jj")
                .version(env!("CARGO_PKG_VERSION"))
                .add_store_factories(store_factories())
                .run()
        })
        .expect("a thread for the command line could not be started");

    // A command line that panicked has said why on its way out, and what is left here is to end with
    // a failure rather than with the zero a joined thread with a value would be read as.
    run.join().unwrap_or(PANIC).into()
}
