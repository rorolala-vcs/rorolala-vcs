use mingling::{
    macros::{buffer, command, help, metadata, r_eprintln},
    metadata::Description,
    res::{ResCurrentDir, ResExitCode},
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::cmd_create::EntryCreate;
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

#[command(entry = EntryInit)]
pub fn init(cwd: &ResCurrentDir) -> EntryCreate {
    // Forward the current working path to `rola create <current path>`
    EntryCreate(vec![cwd.to_path_buf().display().to_string()])
}
