use mingling::{
    LazyRes,
    macros::{command, metadata},
    metadata::Description,
};
use rorolala_cli_setups::ResVault;
use rust_i18n::t;

#[command(entry = EntryCreate)]
pub fn create(vault: &mut LazyRes<ResVault>) {
    if vault.get_ref().exist() {}
}

#[metadata(EntryCreate)]
pub fn desc_create() -> Description {
    t!("cmd_create_description").to_string().into()
}
