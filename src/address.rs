//! Parsing the address a Vault answers at, as the C ABI reads it.
//!
//! A Vault is reached by one of three spellings: a bare `host:port`, a `rola://host:port/namespace`
//! link, and the `user@host:path` an SSH Vault is written as. The protocol reads all three into one
//! [`VaultAddress`], and what this adds is the crossing: the handle a C caller reads the parts of,
//! and the error naming the text that would not read.

use rorolala_errors::AddrError;
use rorolala_protocol::VaultAddress;
use rorolala_utils_lazyffi::lazyffi;

/// Parses `text` as the address a Vault answers at.
///
/// What a caller does with the parts is what `rola_vault_address_raw` and its siblings are for: the
/// link as it was written, the host, the port, the kind, and the account and path of an SSH one.
///
/// # FFI
///
/// Returns a result: `Ok` is a `RolaVaultAddress *`, owned, released with
/// `free_rola_vault_address`; `Err` is a `RolaAddrError *`, owned, released with
/// `free_rola_addr_error`, and it names the target that would not read as an address.
///
/// # Errors
///
/// Returns [`AddrError`] naming `text` when it names no address this build reads.
#[lazyffi(export = rola_vault_address_parse)]
pub fn parse_address(text: &str) -> Result<VaultAddress, AddrError> {
    VaultAddress::parse(text)
}

#[cfg(test)]
mod tests {
    use super::parse_address;

    #[test]
    fn every_spelling_the_protocol_knows_reads() {
        for text in [
            "127.0.0.1:9270",
            "rola://127.0.0.1:9270/studio",
            "alice@example.test:/srv/vault",
        ] {
            assert!(parse_address(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn what_is_not_an_address_names_what_was_written() {
        let error = parse_address("not an address").expect_err("it names no address");

        assert!(error.to_string().contains("not an address"), "{error}");
    }
}
