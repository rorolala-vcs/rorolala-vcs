use mingling::{LazyRes, macros::command};
use rorolala_cli_setups::ResVault;

#[command]
pub fn listen(vault: &mut LazyRes<ResVault>) {
    if vault.get_ref().exist() {}
}
