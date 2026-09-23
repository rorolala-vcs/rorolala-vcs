use mingling::{
    macros::{buffer, command, help, metadata, r_eprintln},
    metadata::Description,
    res::{ResCurrentDir, ResExitCode},
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::cmd_create::StateCreatePath;
use crate::exit_codes::EC_HELP;

#[help(buffer)]
pub fn help_init(_: EntryInit, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("init.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInit)]
pub fn desc_init() -> Description {
    t!("init.cmd_init_description").to_string().into()
}

#[command(node = "init")]
pub fn init(cwd: &ResCurrentDir) -> StateCreatePath {
    // Forward the current working path to the branch `rola create <current path>` would take,
    // so a creation is described in one place and reached from two.
    StateCreatePath::from(cwd.to_path_buf())
}
