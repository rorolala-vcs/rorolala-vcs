//! Addressing the daemon an action is spoken to, and the Vault an address names.
//!
//! There are two spellings for where a Vault is. One is a bare address — a host, a host and a
//! port, or a port alone — which says where a daemon answers and nothing about which Vault
//! under it is meant. The other is the `rola://` link, which says both:
//!
//! ```text
//! rola://<host>[:<port>]/<sub-vault>
//! ```
//!
//! A [`VaultAddress`] is the second, and every form the first takes reads as one: what is left
//! out is filled from what it would otherwise have to be, so an address written without a port
//! takes the port a Vault listens on, and one written without a sub-vault names the root.
//!
//! A host is an ip or a name — `127.0.0.1`, `[::1]`, `localhost`, `example.com`. A name is not
//! resolved here: it is kept as the name it is and left to whatever dials the daemon, so an
//! address stays the address it was written as, whatever a resolver would make of it.

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use arg_picker::{PickerArgResult, SinglePickable};
use rorolala_errors::AddrError;
use rorolala_utils_constants::{ROOT_SUB_VAULT, VAULT_DEFAULT_PORT};
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

/// The ip a port alone is taken to be on.
///
/// An address written without an ip is one a caller means to be local: a port names where on
/// this machine to knock, and writing the ip it is on as well would only be saying `127.0.0.1`
/// in more words.
const LOCALHOST: &str = "127.0.0.1";

/// The scheme a full Vault link is written with.
const SCHEME: &str = "rola://";

/// A Vault named by the link it answers at.
///
/// The parts say where to knock and which Vault is being asked for: `addr` and `port` are the
/// daemon, and `sub_vault` is the Vault under the one it serves. `raw` is what the caller wrote,
/// kept so that what was asked for can be handed back as it was asked, and it is not what the
/// other three are read from once this has been built.
///
/// Every form an address is written in reads as one of these — see [`parse`](Self::parse) — and
/// what comes back is the same kind of thing whichever was written, so a caller never has to
/// ask which spelling it was given.
#[lazyffi(export = RolaVaultAddress)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VaultAddress {
    /// What the caller wrote, as it was written.
    raw: String,
    /// The host the daemon answers at, as an ip or a name.
    addr: String,
    /// The port the daemon answers at.
    port: u16,
    /// The Vault under the daemon's own, or [`ROOT_SUB_VAULT`] for the one it serves.
    sub_vault: String,
}

impl PartialEq for VaultAddress {
    /// Two addresses are the same when they name the same place.
    ///
    /// [`raw`](Self::raw) is not part of that. It is what the caller happened to write, and the
    /// same address is written `10.0.0.1` by one caller and `rola://10.0.0.1/` by another — so a
    /// configuration that wrote one away and read it back holds the address it stored, whatever
    /// spelling the file came back in.
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr && self.port == other.port && self.sub_vault == other.sub_vault
    }
}

impl Eq for VaultAddress {}

impl VaultAddress {
    /// Reads `text` as an address, filling in what it leaves out.
    ///
    /// What is accepted, and what each is taken to mean:
    ///
    /// | written                  | taken as                                          |
    /// | ------------------------ | ------------------------------------------------- |
    /// | `host`                   | that host, the default port, the root Vault       |
    /// | `port`                   | `127.0.0.1`, that port, the root Vault            |
    /// | `host:port`              | that host and port, the root Vault                |
    /// | `rola://host:port/sub`   | all of it                                         |
    /// | `rola://host/sub`        | that host, the default port, that sub-vault       |
    /// | `rola://host:port`       | that host and port, the root Vault                |
    ///
    /// A host is an ip or a name, and a name is kept as it was written rather than resolved.
    ///
    /// # Errors
    ///
    /// Returns an [`AddrError`] naming `text` when it names neither: what is left of a link
    /// after the scheme is a host, a host and a port, or a port, and anything else is refused
    /// rather than guessed at.
    pub fn parse(text: &str) -> Result<Self, AddrError> {
        // A link says which sub-vault and an address does not; without the scheme the whole of
        // it is the authority, and what is under it is the Vault the daemon serves.
        let (authority, sub_vault) =
            text.strip_prefix(SCHEME)
                .map_or((text, ROOT_SUB_VAULT), |link| match link.split_once('/') {
                    Some((authority, sub)) if !sub.is_empty() => (authority, sub),
                    Some((authority, _)) => (authority, ROOT_SUB_VAULT),
                    None => (link, ROOT_SUB_VAULT),
                });

        let (addr, port) =
            read_authority(authority).ok_or_else(|| AddrError::new(text.to_string()))?;

        Ok(Self {
            raw: text.to_string(),
            addr,
            port,
            sub_vault: sub_vault.to_string(),
        })
    }

    /// What the caller wrote, as it was written.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The host the daemon answers at.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.addr
    }

    /// The port the daemon answers at.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// The Vault under the daemon's own that is being addressed.
    #[must_use]
    pub fn sub(&self) -> &str {
        &self.sub_vault
    }

    /// Names the host the daemon answers at.
    ///
    /// What is written here is not read back — an address is read once, when it is parsed — so
    /// a change is the caller's to make sense of, and [`raw`](Self::raw) still holds what was
    /// written before it.
    pub fn set_address(&mut self, address: impl Into<String>) {
        self.addr = address.into();
    }

    /// Names the port the daemon answers at.
    pub const fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    /// Names the Vault under the daemon's own that is being addressed.
    pub fn set_sub(&mut self, sub_vault: impl Into<String>) {
        self.sub_vault = sub_vault.into();
    }

    /// The host as one a port may be written after.
    ///
    /// An ipv6 address is bracketed: the colons it is written with would otherwise read as the
    /// ones separating it from the port. A name is written as it is, since none of them carry a
    /// colon to be taken for one.
    fn host(&self) -> String {
        match self.addr.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => format!("[{}]", self.addr),
            Ok(IpAddr::V4(_)) | Err(_) => self.addr.clone(),
        }
    }

    /// The daemon's address written as the authority of a link: the host, and the port after it.
    ///
    /// It is where to knock and nothing more — which Vault under the daemon is meant is not part
    /// of it, and is said to the daemon separately. This is the inverse of the reading a link's
    /// authority gets in [`parse`](Self::parse).
    #[must_use]
    pub fn authority(&self) -> String {
        format!("{}:{}", self.host(), self.port)
    }
}

/// The host and port `authority` names, if it names either.
///
/// A host and a port is read as it is, a host alone takes the default port, and a port alone is a
/// port on [`LOCALHOST`]. A host is an ip or a name, and a name is kept as it was written rather
/// than resolved: this decides what was said, not what it turns out to point at.
fn read_authority(authority: &str) -> Option<(String, u16)> {
    if let Ok(address) = authority.parse::<SocketAddr>() {
        return Some((address.ip().to_string(), address.port()));
    }

    if let Ok(ip) = authority.parse::<IpAddr>() {
        return Some((ip.to_string(), VAULT_DEFAULT_PORT));
    }

    // A number that is not a port is not a host either: what was meant by a bare number is a
    // port, and one no machine has is refused rather than read as a name spelled in digits.
    if authority.bytes().all(|byte| byte.is_ascii_digit()) {
        return authority
            .parse::<u16>()
            .ok()
            .map(|port| (LOCALHOST.to_string(), port));
    }

    if let Some((host, port)) = authority.rsplit_once(':') {
        return port
            .parse::<u16>()
            .ok()
            .filter(|_| is_host(host))
            .map(|port| (host.to_string(), port));
    }

    if is_host(authority) {
        return Some((authority.to_string(), VAULT_DEFAULT_PORT));
    }

    None
}

/// Whether `name` reads as a host a daemon could be dialled at.
///
/// A host is written with the letters, digits, hyphens, dots and underscores a name or an ip is
/// made of — `localhost` is the one name every machine knows. An empty name, and anything
/// carrying a separator, whitespace or the like, is not one, and is refused here rather than
/// handed on for a resolver to fail over.
fn is_host(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-' || byte == b'_'
        })
}

impl fmt::Display for VaultAddress {
    /// Writes the link the address names.
    ///
    /// The port is written only when it is not the one an address without a port means: what is
    /// left out is what a reader would have to write the same number to say, and a link is read
    /// back as the address it spells either way.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{SCHEME}{}", self.host())?;

        if self.port != VAULT_DEFAULT_PORT {
            write!(formatter, ":{}", self.port)?;
        }

        write!(formatter, "/{}", self.sub_vault)
    }
}

impl FromStr for VaultAddress {
    type Err = AddrError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl TryFrom<String> for VaultAddress {
    type Error = AddrError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}

impl TryFrom<&str> for VaultAddress {
    type Error = AddrError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl From<VaultAddress> for String {
    /// The link, as [`Display`](fmt::Display) writes it.
    ///
    /// This is how an address is written down: a Workspace keeps the link a caller would write,
    /// so that its configuration and a command line read the same way.
    fn from(address: VaultAddress) -> Self {
        address.to_string()
    }
}

impl From<SocketAddr> for VaultAddress {
    /// The root of the Vault at `address`.
    ///
    /// An address says where a daemon is and nothing about which Vault under it is meant, so
    /// what it becomes names the one that daemon serves. This is how an address that was taken
    /// apart and put back together — one read from a configuration, say — is read as the same
    /// kind of thing a caller wrote.
    fn from(address: SocketAddr) -> Self {
        Self {
            raw: address.to_string(),
            addr: address.ip().to_string(),
            port: address.port(),
            sub_vault: ROOT_SUB_VAULT.to_string(),
        }
    }
}

impl SinglePickable for VaultAddress {
    /// Reads one word of a command line as the address it names.
    ///
    /// It is [`parse`](VaultAddress::parse) and nothing else: what a caller can write on a
    /// command line is what an address is written as anywhere, so the two cannot come apart.
    /// A word that names no address is [`NotFound`](PickerArgResult::NotFound) — the picker's
    /// way of saying the argument was not one it could read — rather than a value made up to
    /// stand for one.
    fn pick_single(text: Option<&str>) -> PickerArgResult<Self> {
        text.map_or(PickerArgResult::NotFound, |text| Self::parse(text).into())
    }
}

#[lazyffi]
impl VaultAddress {
    /// The link the caller wrote, as it was written.
    #[lazyffi(export = vault_address_read_raw)]
    #[must_use]
    pub fn read_raw(&self) -> String {
        self.raw.clone()
    }

    /// The link's host, as the ABI spells it.
    #[lazyffi(export = vault_address_read_addr)]
    #[must_use]
    pub fn read_addr(&self) -> String {
        self.addr.clone()
    }

    /// The link's port, as the ABI spells it.
    #[lazyffi(export = vault_address_read_port)]
    #[must_use]
    pub const fn read_port(&self) -> u16 {
        self.port
    }
}

#[cfg(test)]
mod tests {
    use super::VaultAddress;
    use rorolala_utils_constants::{ROOT_SUB_VAULT, VAULT_DEFAULT_PORT};
    use std::net::SocketAddr;

    /// The parts of `text`, read as an address.
    fn parts(text: &str) -> (String, u16, String) {
        let address = VaultAddress::parse(text).unwrap();

        (
            address.address().to_string(),
            address.port(),
            address.sub().to_string(),
        )
    }

    #[test]
    fn a_host_is_read_as_it_was_written() {
        // An ip and a port, and an ip alone, which takes the port a Vault listens on.
        assert_eq!(
            parts("127.0.0.1:7000"),
            ("127.0.0.1".to_string(), 7000, ROOT_SUB_VAULT.to_string())
        );
        assert_eq!(
            parts("127.0.0.1"),
            (
                "127.0.0.1".to_string(),
                VAULT_DEFAULT_PORT,
                ROOT_SUB_VAULT.to_string()
            )
        );

        // A name is a host too, and is kept as it was written rather than resolved.
        assert_eq!(
            parts("example.com"),
            (
                "example.com".to_string(),
                VAULT_DEFAULT_PORT,
                ROOT_SUB_VAULT.to_string()
            )
        );
        assert_eq!(
            parts("localhost:7000"),
            ("localhost".to_string(), 7000, ROOT_SUB_VAULT.to_string())
        );
    }

    #[test]
    fn a_bare_address_names_the_root_of_the_vault_it_answers_at() {
        // An ip alone takes the port a Vault listens on and the Vault the daemon serves.
        assert_eq!(
            parts("127.0.0.1"),
            (
                "127.0.0.1".to_string(),
                VAULT_DEFAULT_PORT,
                ROOT_SUB_VAULT.to_string()
            )
        );

        // An ip and a port says the port too.
        assert_eq!(
            parts("10.0.0.1:7000"),
            ("10.0.0.1".to_string(), 7000, ROOT_SUB_VAULT.to_string())
        );

        // A port alone is a port on this machine.
        assert_eq!(
            parts("7000"),
            ("127.0.0.1".to_string(), 7000, ROOT_SUB_VAULT.to_string())
        );
    }

    #[test]
    fn a_link_says_the_sub_vault_beside_the_address() {
        assert_eq!(
            parts("rola://10.0.0.1:7000/vaults/alpha"),
            ("10.0.0.1".to_string(), 7000, "vaults/alpha".to_string())
        );

        // A link without a port takes the default one...
        assert_eq!(
            parts("rola://10.0.0.1/vaults/alpha"),
            (
                "10.0.0.1".to_string(),
                VAULT_DEFAULT_PORT,
                "vaults/alpha".to_string()
            )
        );

        // ...and one without a sub-vault names the root, as an empty one does.
        assert_eq!(
            parts("rola://10.0.0.1:7000"),
            ("10.0.0.1".to_string(), 7000, ROOT_SUB_VAULT.to_string())
        );
        assert_eq!(
            parts("rola://10.0.0.1:7000/"),
            ("10.0.0.1".to_string(), 7000, ROOT_SUB_VAULT.to_string())
        );
    }

    #[test]
    fn an_ipv6_address_is_read_and_written_without_coming_apart() {
        let bare = VaultAddress::parse("::1").unwrap();
        assert_eq!(bare.address(), "::1");
        assert_eq!(bare.port(), VAULT_DEFAULT_PORT);

        // The colons of an ipv6 address would read as the port's, so it is bracketed when a
        // port follows it — including where the port is left out.
        assert_eq!(bare.to_string(), "rola://[::1]/");

        let with_port = VaultAddress::parse("rola://[::1]:7000/sub").unwrap();
        assert_eq!(with_port.address(), "::1");
        assert_eq!(with_port.port(), 7000);
        assert_eq!(with_port.to_string(), "rola://[::1]:7000/sub");
    }

    #[test]
    fn a_link_is_written_with_the_port_only_where_it_is_not_the_default_one() {
        // A bare address is written back as the link it adds up to, port and all...
        let bare = VaultAddress::parse("10.0.0.1").unwrap();
        assert_eq!(bare.to_string(), "rola://10.0.0.1/");

        // ...and a link is written without the port where the port is the one it would have
        // taken anyway.
        let default = VaultAddress::parse("rola://10.0.0.1/vaults/alpha").unwrap();
        assert_eq!(default.to_string(), "rola://10.0.0.1/vaults/alpha");

        let named = VaultAddress::parse("rola://10.0.0.1:7000/vaults/alpha").unwrap();
        assert_eq!(named.to_string(), "rola://10.0.0.1:7000/vaults/alpha");
    }

    #[test]
    fn what_is_written_is_read_back_as_the_same_address() {
        for written in [
            "127.0.0.1",
            "127.0.0.1:7000",
            "7000",
            "rola://10.0.0.1:7000/vaults/alpha",
            "rola://10.0.0.1/vaults/deep/alpha",
            "rola://[::1]:7000/sub",
        ] {
            let address = VaultAddress::parse(written).unwrap();
            let read_back = VaultAddress::parse(&address.to_string()).unwrap();

            assert_eq!(read_back.address(), address.address(), "{written}");
            assert_eq!(read_back.port(), address.port(), "{written}");
            assert_eq!(read_back.sub(), address.sub(), "{written}");
        }
    }

    #[test]
    fn what_was_written_is_kept_as_it_was_written() {
        let address = VaultAddress::parse("10.0.0.1:7000").unwrap();

        // `raw` is the caller's own words, not the link they add up to...
        assert_eq!(address.raw(), "10.0.0.1:7000");
        assert_eq!(address.to_string(), "rola://10.0.0.1:7000/");

        // ...and changing a part does not rewrite it, since nothing reads the parts back out.
        let mut changed = address;
        changed.set_address("10.0.0.2");
        changed.set_port(7001);
        changed.set_sub("vaults/beta");

        assert_eq!(changed.raw(), "10.0.0.1:7000");
        assert_eq!(changed.address(), "10.0.0.2");
        assert_eq!(changed.port(), 7001);
        assert_eq!(changed.sub(), "vaults/beta");
    }

    #[test]
    fn what_names_neither_a_host_nor_a_port_is_refused() {
        for written in [
            "",
            "rola://",
            "99999",
            "10.0.0.1:99999",
            "not a host",
            "rola://a b",
        ] {
            assert!(VaultAddress::parse(written).is_err(), "{written}");
        }

        // What is refused is named back, so a caller can see which word it was.
        assert_eq!(
            VaultAddress::parse("rola://").unwrap_err().target(),
            "rola://"
        );
    }

    #[test]
    fn a_link_may_name_its_host() {
        assert_eq!(
            parts("rola://localhost/sub"),
            (
                "localhost".to_string(),
                VAULT_DEFAULT_PORT,
                "sub".to_string()
            )
        );
        assert_eq!(
            parts("rola://example.com:7000/vaults/alpha"),
            ("example.com".to_string(), 7000, "vaults/alpha".to_string())
        );
    }

    #[test]
    fn an_address_read_the_way_rust_reads_one_comes_out_the_same() {
        let text = "rola://10.0.0.1:7000/vaults/alpha";
        let expected = VaultAddress::parse(text).unwrap();

        assert_eq!(text.parse::<VaultAddress>().unwrap(), expected);
        assert_eq!(VaultAddress::try_from(text.to_string()).unwrap(), expected);
        assert_eq!(VaultAddress::try_from(text).unwrap(), expected);
    }

    #[test]
    fn a_socket_address_becomes_the_root_of_the_vault_it_answers_at() {
        let address: SocketAddr = "10.0.0.1:7000".parse().unwrap();
        let vault = VaultAddress::from(address);

        assert_eq!(vault.address(), "10.0.0.1");
        assert_eq!(vault.port(), 7000);
        assert_eq!(vault.sub(), ROOT_SUB_VAULT);
    }

    #[test]
    fn the_abi_reads_back_the_parts_the_link_was_written_with() {
        let address = VaultAddress::parse("rola://10.0.0.1:7000/vaults/alpha").unwrap();

        assert_eq!(address.read_raw(), "rola://10.0.0.1:7000/vaults/alpha");
        assert_eq!(address.read_addr(), "10.0.0.1");
        assert_eq!(address.read_port(), 7000);
    }

    #[test]
    fn two_spellings_of_one_place_are_one_address() {
        // What was written is not what an address is: the same place reached for in two ways
        // is one address, which is what lets a stored one be compared with a parsed one.
        assert_eq!(
            VaultAddress::parse("10.0.0.1:7000").unwrap(),
            VaultAddress::parse("rola://10.0.0.1:7000/").unwrap()
        );

        // A different place is a different address, whichever spelling it was written in.
        assert_ne!(
            VaultAddress::parse("rola://10.0.0.1:7000/").unwrap(),
            VaultAddress::parse("rola://10.0.0.1:7000/vaults/alpha").unwrap()
        );
    }

    #[test]
    fn the_authority_is_the_daemon_without_the_sub_vault() {
        let root = VaultAddress::parse("10.0.0.1").unwrap();
        assert_eq!(root.authority(), format!("10.0.0.1:{VAULT_DEFAULT_PORT}"));

        // Which Vault under the daemon is meant is not part of where to knock.
        let under = VaultAddress::parse("rola://10.0.0.1:7000/vaults/alpha").unwrap();
        assert_eq!(under.authority(), "10.0.0.1:7000");

        // An ipv6 address is bracketed, so the port after it reads as the port.
        let v6 = VaultAddress::parse("rola://[::1]:7000/sub").unwrap();
        assert_eq!(v6.authority(), "[::1]:7000");

        // A name is written as it is: there is no colon in it to be taken for the port's.
        let named = VaultAddress::parse("rola://example.com/sub").unwrap();
        assert_eq!(
            named.authority(),
            format!("example.com:{VAULT_DEFAULT_PORT}")
        );
    }
}
