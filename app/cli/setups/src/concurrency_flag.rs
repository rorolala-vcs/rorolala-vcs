use std::ops::Deref;

use mingling::{
    ProgramCollect,
    macros::arg,
    picker::{PickerArg, PickerHelper},
    setup::ProgramSetup,
};

/// How many transfers a run carries at once when `--concurrency` names none.
pub const DEFAULT_CONCURRENCY: usize = 4;

/// The most a run may ask to carry at once.
///
/// An upper bound rather than a preference: each lane is a connection the Vault has to serve, so a
/// number a caller has no use for is refused here rather than spent on a daemon that then has to
/// refuse it.
pub const MAX_CONCURRENCY: usize = 64;

/// Global argument selecting how many transfers a run carries at once.
///
/// Usage: `--concurrency 8`
pub const GLOBAL_ARG_CONCURRENCY: PickerArg<'static, String> = arg![concurrency: String];

/// Represents how many transfers the run carries at once, and whether the run named it.
///
/// A Vault reads a named value differently from a default one: what a client asks for is its own
/// number of lanes, while a Vault that is told nothing keeps the number its configuration names —
/// see `DaemonConfig::max_concurrency`. So whether the flag was given travels with the value.
#[derive(Debug, Clone)]
pub struct ResConcurrency {
    concurrency: usize,
    explicit: bool,
}

impl Default for ResConcurrency {
    fn default() -> Self {
        Self {
            concurrency: DEFAULT_CONCURRENCY,
            explicit: false,
        }
    }
}

impl ResConcurrency {
    /// How many transfers the run carries at once.
    #[must_use]
    pub const fn concurrency(&self) -> usize {
        self.concurrency
    }

    /// Whether the run named `--concurrency` rather than leaving it to the default.
    #[must_use]
    pub const fn explicit(&self) -> bool {
        self.explicit
    }
}

impl Deref for ResConcurrency {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.concurrency
    }
}

/// A [`ProgramSetup`] implementation that registers how many transfers a run carries at once.
///
/// `--concurrency` is how many lanes a transfer of many keys is spread over: each lane is a
/// connection of its own, so the Vault serves them at once. A value that is not a number in range
/// falls back to the default rather than stopping the run: the flag chooses how fast work moves,
/// not whether it moves.
pub struct ConcurrencySetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for ConcurrencySetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let named = program.pick_argument(&GLOBAL_ARG_CONCURRENCY);
        let explicit = named.is_some();
        let concurrency = named
            .and_then(|asked| asked.parse::<usize>().ok())
            .map_or(DEFAULT_CONCURRENCY, |asked| asked.clamp(1, MAX_CONCURRENCY));

        program.with_resource(ResConcurrency {
            concurrency,
            explicit,
        });
    }
}
