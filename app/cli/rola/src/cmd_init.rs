use mingling::{macros::command, res::ResCurrentDir};

use crate::cmd_create::EntryCreate;

#[command(entry = EntryInit)]
pub fn init(cwd: &ResCurrentDir) -> EntryCreate {
    EntryCreate(vec![cwd.to_path_buf().display().to_string()])
}
