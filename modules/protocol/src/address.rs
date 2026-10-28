//! Addressing the daemon an action is spoken to, and the namespace a request names.
//!
//! There are three spellings for where a Vault is. One is a bare address — a host, a host and a
//! port, or a port alone — which says where a daemon answers and nothing about which namespace
//! under it is meant. Another is the `rola://` link, which says both:
//!
//! ```text
//! rola://<host>[:<port>]/<namespace>[/<sub-vault>]
//! ```
//!
//! The third is a Vault reached over SSH, written the way `scp` writes one, with the port
//! between the host and the path when the port is not the one SSH itself would take:
//!
//! ```text
//! <user>@<host>[:<port>]:<path>
//! ```
//!
//! A [`VaultAddress`] is one of these three, read into the [`Link`] it names. Every form the bare
//! address takes reads as a daemon link: what is left out is filled from what it would otherwise
//! have to be, so an address written without a port takes the port a daemon listens on. The
//! namespace is the first path segment; anything after it names a Vault under the one the namespace
//! was registered for.
//!
//! A host is an ip or a name — `127.0.0.1`, `[::1]`, `localhost`, `example.com`. A name is not
//! resolved here: it is kept as the name it is and left to whatever dials the daemon, so an
//! address stays the address it was written as, whatever a resolver would make of it.
//!
//! An SSH address names no namespace: its path *is* the Vault, so a request over one carries the
//! root and nothing under. What is written after the host is a path on the far side of the
//! connection, so it is the one part of an address this program does not read at all — it is
//! quoted and handed to `ssh` as it was written.

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

/// The separator between the account an SSH session runs as and the host it reaches.
const SSH_USER_SEPARATOR: char = '@';

/// A Vault named by the link it answers at.
///
/// The parts say where to knock and which Vault is being asked for: `link` is the daemon or the
/// SSH session that carries a request, and `sub_vault` is what a request names under a daemon —
/// a namespace, or a namespace and a Vault below the one that namespace was registered for.
/// `raw` is what the caller wrote, kept so that what was asked for can be handed back as it was
/// asked, and it is not what the other two are read from once this has been built.
///
/// Every form an address is written in reads as one of these — see [`parse`](Self::parse) — and
/// what comes back is the same kind of thing whichever was written, so a caller never has to
/// ask which spelling it was given. Which *kind* it is, though, is [`link`](Self::link)'s to
/// say: a daemon is dialled over TCP, and an SSH session is not reached here at all.
#[lazyffi(export = RolaVaultAddress)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VaultAddress {
    /// What the caller wrote, as it was written.
    raw: String,
    /// Where the request goes.
    link: Link,
    /// The namespace under the daemon that is being addressed, and the Vault below it when one
    /// is named.
    ///
    /// It is the first path segment of the link and whatever follows it: `alpha` names the
    /// namespace, `alpha/vaults/beta` names the Vault that namespace holds under `beta`. An SSH
    /// address names no namespace — its path is the Vault — so it is always empty.
    sub_vault: String,
}

/// Where a Vault is reached: a daemon on this network, or a daemon behind an SSH session.
///
/// What is read from an address is one of these, and what a caller does with it is what tells
/// them apart: a [`Daemon`](Self::Daemon) is a TCP connection to make here, while an
/// [`Ssh`](Self::Ssh) is a process to start and a request to carry through its standard input
/// and output. The protocol that runs over either is the same one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// A daemon listening on a port.
    Daemon {
        /// The host the daemon answers at, as an ip or a name.
        host: String,
        /// The port the daemon answers at.
        port: u16,
    },
    /// A daemon reached over an SSH session, serving the Vault at a path on the far side.
    Ssh {
        /// The account the session runs as on the far side.
        user: String,
        /// The host the session is made to, as an ip or a name.
        host: String,
        /// The port the session is made to, when one was written: `None` leaves it to SSH's own
        /// configuration, and to the port SSH itself defaults to.
        port: Option<u16>,
        /// The directory the Vault sits at on the far side, as it was written.
        ///
        /// A relative path is relative to the working directory the session starts in, which is
        /// the far side's home directory: what it names is the far side's to resolve, so nothing
        /// here can say whether it is one.
        path: String,
    },
}

impl PartialEq for VaultAddress {
    /// Two addresses are the same when they name the same place.
    ///
    /// [`raw`](Self::raw) is not part of that. It is what the caller happened to write, and the
    /// same address is written `10.0.0.1` by one caller and `rola://10.0.0.1/` by another — so a
    /// configuration that wrote one away and read it back holds the address it stored, whatever
    /// spelling the file came back in. What is compared is the link and the namespace under it:
    /// an SSH link is where a request goes as much as a host and a port are.
    fn eq(&self, other: &Self) -> bool {
        self.link == other.link && self.sub_vault == other.sub_vault
    }
}

impl Eq for VaultAddress {}

impl VaultAddress {
    /// Reads `text` as an address, filling in what it leaves out.
    ///
    /// What is accepted, and what each is taken to mean:
    ///
    /// | written                 | taken as                                          |
    /// | ----------------------- | ------------------------------------------------- |
    /// | `host`                  | that host, the default port, no namespace         |
    /// | `port`                  | `127.0.0.1`, that port, no namespace              |
    /// | `host:port`             | that host and port, no namespace                  |
    /// | `rola://host:port/ns`   | all of it                                         |
    /// | `rola://host/ns/sub`    | that host, the default port, namespace and Vault  |
    /// | `host:port/ns`          | the bare authority, then the namespace            |
    /// | `host/ns/sub`           | the same, with the default port                   |
    /// | `user@host:path`        | an SSH session to that host, serving that path    |
    /// | `user@host:port:path`   | the same, on that port                            |
    ///
    /// A host is an ip or a name, and a name is kept as it was written rather than resolved.
    ///
    /// What an SSH address may be is narrower, because its parts are not all read the same way:
    /// an account is letters, digits, `.`, `_` and `-`; a host is what a host is anywhere else,
    /// or an ipv6 address in brackets; a port, when it is written, is a number between 1 and
    /// 65535; and a path may carry anything but what would make it ambiguous — a second colon is
    /// refused rather than guessed at — and anything that would end the line it is handed to
    /// `ssh` on. A path starting with `-` is refused too, since what is written with it on the
    /// far side is a command line.
    ///
    /// # Errors
    ///
    /// Returns an [`AddrError`] naming `text` when it names no daemon: what is left of a link
    /// before its first `/` is a host, a host and a port, or a port, and anything else is refused
    /// rather than guessed at.
    pub fn parse(text: &str) -> Result<Self, AddrError> {
        // The `rola://` scheme is a daemon link and nothing else: an SSH address is not written
        // with one, so an `@` after the scheme reads as an address that names no daemon — which
        // is what it is, since a daemon link has no account.
        if let Some(rest) = text.strip_prefix(SCHEME) {
            return Self::parse_daemon(text, rest);
        }

        // An `@` before the first `:` is what tells an SSH address from every other spelling: no
        // daemon address may carry one, since a host is written without it. The order matters —
        // reading the first `/` first would split `user@host:path/to/vault` into an authority and
        // a namespace, which is not what was written.
        if is_ssh(text) {
            return Self::parse_ssh(text);
        }

        Self::parse_daemon(text, text)
    }

    /// Reads the daemon link `text` spells, `rest` being `text` without the scheme.
    ///
    /// The first `/` separates the daemon from what is named under it, whether or not the scheme
    /// was written: `host:port` and `rola://host:port` are the same authority, and
    /// `host:port/ns` says what `rola://host:port/ns` says. What has no `/` names the daemon and
    /// no namespace at all.
    fn parse_daemon(text: &str, rest: &str) -> Result<Self, AddrError> {
        let (authority, sub_vault) = match rest.split_once('/') {
            Some((authority, sub)) if !sub.is_empty() => (authority, sub),
            Some((authority, _)) => (authority, ROOT_SUB_VAULT),
            None => (rest, ROOT_SUB_VAULT),
        };

        let (host, port) =
            read_authority(authority).ok_or_else(|| AddrError::new(text.to_string()))?;

        Ok(Self {
            raw: text.to_string(),
            link: Link::Daemon { host, port },
            sub_vault: sub_vault.to_string(),
        })
    }

    /// Reads the SSH link `text` spells.
    ///
    /// Nothing is filled in here: what an SSH address leaves out is left out — a port with no
    /// number is not the default port, it is SSH's own to choose — so a part that was not written
    /// cannot be mistaken for one that was.
    fn parse_ssh(text: &str) -> Result<Self, AddrError> {
        read_ssh(text)
            .map(|link| Self {
                raw: text.to_string(),
                link,
                // An SSH address names the Vault by its path, so there is no namespace under it
                // for a request to carry.
                sub_vault: ROOT_SUB_VAULT.to_string(),
            })
            .ok_or_else(|| AddrError::new(text.to_string()))
    }

    /// What the caller wrote, as it was written.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Where the request goes: a daemon to dial here, or an SSH session to start.
    #[must_use]
    pub const fn link(&self) -> &Link {
        &self.link
    }

    /// The host the daemon, or the SSH session, answers at.
    ///
    /// An SSH address names an account as well as a host; the account is part of the
    /// [`link`](Self::link) and not of this, since what a host is is the same either way.
    #[must_use]
    pub fn address(&self) -> &str {
        self.link.host()
    }

    /// The namespace under the daemon that is being addressed, and the Vault below it when one
    /// is named.
    ///
    /// An SSH address names the Vault by its path and has no namespace under it, so it is the
    /// root: a request made through one carries nothing, which is what the far side requires.
    #[must_use]
    pub fn sub(&self) -> &str {
        &self.sub_vault
    }
}

impl Link {
    /// The host this link reaches, as an ip or a name.
    ///
    /// It is not a `const fn`: what it hands back is the text of a `String` reached through a
    /// reference, and that deref is not one a constant evaluation may make.
    #[must_use]
    pub fn host(&self) -> &str {
        match self {
            Self::Daemon { host, .. } | Self::Ssh { host, .. } => host,
        }
    }

    /// The host written so that a port may follow it.
    ///
    /// An ipv6 address is bracketed: the colons it is written with would otherwise read as the
    /// ones separating it from the port. A name is written as it is, since none of them carry a
    /// colon to be taken for one. It is written here rather than by whoever dials the link, so
    /// that the address a connection is made to and the address a configuration keeps are the
    /// same spelling of the same host.
    #[must_use]
    pub fn written_host(&self) -> String {
        let host = self.host();

        match host.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => format!("[{host}]"),
            Ok(IpAddr::V4(_)) | Err(_) => host.to_owned(),
        }
    }
}

impl fmt::Display for Link {
    /// Writes the link as an address: a daemon's host and port, or an SSH session's account,
    /// host, port and path.
    ///
    /// A daemon's port is always written here, since the one thing this cannot say is which
    /// number was left out — an address says that by writing the link it adds up to, and that is
    /// [`VaultAddress`]'s to write.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Daemon { port, .. } => write!(formatter, "{}:{port}", self.written_host()),
            Self::Ssh {
                user, port, path, ..
            } => {
                write!(formatter, "{user}@{}", self.written_host())?;

                if let Some(port) = port {
                    write!(formatter, ":{port}")?;
                }

                write!(formatter, ":{path}")
            }
        }
    }
}

/// Whether `text` is written as an SSH address rather than as anything else.
///
/// An account is what says so: no daemon address may carry an `@`, since a host is written
/// without one, so one before the first `:` is the only thing a spelling has to have to be an SSH
/// one. The first `:` is what it is compared against rather than the first `/`: the separator a
/// host and a port share comes before any path either kind of address names.
fn is_ssh(text: &str) -> bool {
    match (text.find(SSH_USER_SEPARATOR), text.find(':')) {
        (Some(at), Some(colon)) => at < colon,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// The SSH link `text` spells, if it spells one.
///
/// Every part is read and checked here rather than in the caller, so that what an address may be
/// is written once: what comes back is a link whose parts are all of them the kind a session is
/// started with.
fn read_ssh(text: &str) -> Option<Link> {
    let (user, rest) = text.split_once(SSH_USER_SEPARATOR)?;

    if !is_ssh_user(user) {
        return None;
    }

    let (host, rest) = read_ssh_host(rest)?;
    let (port, path) = read_ssh_port(rest)?;

    if !is_ssh_path(path) {
        return None;
    }

    Some(Link::Ssh {
        user: user.to_string(),
        host,
        port,
        path: path.to_string(),
    })
}

/// The host an SSH address names, and what follows it.
///
/// A bracketed ipv6 address is read between the brackets, so the colons it carries are not taken
/// for the separator that ends the host. Anything else is read up to the first `:`, which is
/// where a port or a path starts.
fn read_ssh_host(rest: &str) -> Option<(String, &str)> {
    if let Some(bracketed) = rest.strip_prefix('[') {
        let (inside, after) = bracketed.split_once(']')?;

        // The brackets are what says the address inside them is an ipv6 one: an ipv4 address is
        // written without them, so one written with them is not an address this reads.
        match inside.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => {}
            Ok(IpAddr::V4(_)) | Err(_) => return None,
        }

        return Some((inside.to_string(), after.strip_prefix(':')?));
    }

    let (host, rest) = rest.split_once(':')?;

    is_host(host).then(|| (host.to_string(), rest))
}

/// The port an SSH address names and the path after it, when no port was written being `None`.
///
/// Two colons after the host are refused rather than read as one of the two readings: `a:b:c`
/// says nothing about which of `a` and `b` is the port, and guessing is what a person writing a
/// path with a colon in it would be met with.
fn read_ssh_port(rest: &str) -> Option<(Option<u16>, &str)> {
    let Some((port, path)) = rest.split_once(':') else {
        return Some((None, rest));
    };

    if path.contains(':') {
        return None;
    }

    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let port = port.parse::<u16>().ok()?;

    // Nothing listens on 0 in SSH's sense of the word, and a port is written to say which one,
    // so zero is a number that was written wrongly rather than one that means something.
    (port > 0).then_some((Some(port), path))
}

/// Whether `user` is one an SSH session may be started as.
///
/// Letters, digits, `.`, `_` and `-`, and not a `-` to start with: what is written here is
/// taken as the value of an option rather than parsed again, so it only has to be a name that
/// cannot be read as one.
fn is_ssh_user(user: &str) -> bool {
    !user.is_empty()
        && !user.starts_with('-')
        && user
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Whether `path` is one an SSH session may be asked to serve.
///
/// It is handed on to the far side as an argument to a command, so it cannot carry what would end
/// that command (a colon, refused by its reader for being ambiguous, and the newlines), and it
/// cannot start with a `-`, which the far side's program would read as a flag rather than as the
/// path it is. Everything else — the spaces, the quotes, the characters a shell would act on —
/// is allowed, and escaped by whoever writes the command.
fn is_ssh_path(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('-') && !path.contains([':', '\0', '\n', '\r'])
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
    /// A daemon link leaves its port out when it is the one an address without a port means: what
    /// is left out is what a reader would have to write the same number to say, and a link is
    /// read back as the address it spells either way. An SSH link leaves out only what was left
    /// out — a port that was not written is not a port that was — since the path it does write is
    /// the far side's to read.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.link {
            Link::Daemon { port, .. } => {
                write!(formatter, "{SCHEME}{}", self.link.written_host())?;

                if *port != VAULT_DEFAULT_PORT {
                    write!(formatter, ":{port}")?;
                }

                write!(formatter, "/{}", self.sub_vault)
            }
            // An SSH link is written as the whole address, path and all: it has no namespace
            // after a slash to say anything more with.
            Link::Ssh { .. } => write!(formatter, "{}", self.link),
        }
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
            link: Link::Daemon {
                host: address.ip().to_string(),
                port: address.port(),
            },
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

#[lazyffi(export = rola_vault_address_)]
impl VaultAddress {
    /// The link the caller wrote, as it was written.
    #[lazyffi(export = vault_address_read_raw)]
    #[must_use]
    pub fn read_raw(&self) -> String {
        self.raw.clone()
    }

    /// The link's host, as the ABI spells it.
    ///
    /// It is the host alone, without the account an SSH address names: what an SSH address adds
    /// to a host is [`read_ssh_user`](Self::read_ssh_user)'s to say.
    #[lazyffi(export = vault_address_read_addr)]
    #[must_use]
    pub fn read_addr(&self) -> String {
        self.address().to_string()
    }

    /// The link's port, as the ABI spells it.
    ///
    /// An SSH address names a port only when one was written, and one that was not is SSH's own
    /// to choose — so nothing is written where a number is expected, which is what `0` is here:
    /// no session is made to port 0.
    #[lazyffi(export = vault_address_read_port)]
    #[must_use]
    pub const fn read_port(&self) -> u16 {
        match &self.link {
            Link::Daemon { port, .. } => *port,
            Link::Ssh { port, .. } => match port {
                Some(port) => *port,
                None => 0,
            },
        }
    }

    /// Which kind of link the address names, as the ABI spells it.
    ///
    /// What a caller branches on: a daemon is a TCP connection to make, and an SSH session is a
    /// process to start. It is `0` for a daemon and `1` for an SSH session, and
    /// [`read_ssh_user`](Self::read_ssh_user) and [`read_ssh_path`](Self::read_ssh_path) are
    /// read after it says which.
    #[lazyffi(export = vault_address_read_kind)]
    #[must_use]
    pub const fn read_kind(&self) -> u8 {
        match self.link {
            Link::Daemon { .. } => 0,
            Link::Ssh { .. } => 1,
        }
    }

    /// The account an SSH address names, as the ABI spells it.
    ///
    /// It is empty for a daemon link, which names no account: what a caller reads it for is known
    /// from [`read_kind`](Self::read_kind) before it is asked.
    #[lazyffi(export = vault_address_read_ssh_user)]
    #[must_use]
    pub fn read_ssh_user(&self) -> String {
        match &self.link {
            Link::Ssh { user, .. } => user.clone(),
            Link::Daemon { .. } => String::new(),
        }
    }

    /// The directory an SSH address names, as the ABI spells it.
    ///
    /// It is empty for a daemon link, which names a namespace rather than a path: what a daemon
    /// link has to say about where a Vault is is the namespace alone.
    #[lazyffi(export = vault_address_read_ssh_path)]
    #[must_use]
    pub fn read_ssh_path(&self) -> String {
        match &self.link {
            Link::Ssh { path, .. } => path.clone(),
            Link::Daemon { .. } => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Link, VaultAddress};
    use rorolala_utils_constants::{ROOT_SUB_VAULT, VAULT_DEFAULT_PORT};
    use std::net::SocketAddr;

    /// The parts of `text`, read as a daemon link.
    fn parts(text: &str) -> (String, u16, String) {
        let address = VaultAddress::parse(text).unwrap();

        let (host, port) = match address.link() {
            Link::Daemon { host, port } => (host.clone(), *port),
            Link::Ssh { .. } => panic!("{text} names an SSH session, not a daemon"),
        };

        (host, port, address.sub().to_string())
    }

    /// The parts of `text`, read as an SSH link.
    fn ssh(text: &str) -> (String, String, Option<u16>, String) {
        let address = VaultAddress::parse(text).unwrap();

        match address.link() {
            Link::Ssh {
                user,
                host,
                port,
                path,
            } => (user.clone(), host.clone(), *port, path.clone()),
            Link::Daemon { .. } => panic!("{text} names a daemon, not an SSH session"),
        }
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
    fn a_link_says_the_namespace_beside_the_address() {
        assert_eq!(
            parts("rola://10.0.0.1:7000/alpha"),
            ("10.0.0.1".to_string(), 7000, "alpha".to_string())
        );

        // A link without a port takes the default one...
        assert_eq!(
            parts("rola://10.0.0.1/alpha/vaults/beta"),
            (
                "10.0.0.1".to_string(),
                VAULT_DEFAULT_PORT,
                "alpha/vaults/beta".to_string()
            )
        );

        // ...and one without a namespace names the daemon and nothing under it, as an empty one
        // does.
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
    fn an_address_written_without_a_scheme_names_the_namespace_too() {
        // The first `/` is where the daemon ends whether or not the scheme was written.
        assert_eq!(
            parts("10.0.0.1:7000/alpha"),
            ("10.0.0.1".to_string(), 7000, "alpha".to_string())
        );
        assert_eq!(
            parts("10.0.0.1/alpha/vaults/beta"),
            (
                "10.0.0.1".to_string(),
                VAULT_DEFAULT_PORT,
                "alpha/vaults/beta".to_string()
            )
        );
        assert_eq!(
            parts("7000/alpha"),
            ("127.0.0.1".to_string(), 7000, "alpha".to_string())
        );
        assert_eq!(
            parts("localhost/alpha"),
            (
                "localhost".to_string(),
                VAULT_DEFAULT_PORT,
                "alpha".to_string()
            )
        );

        // A trailing separator names the daemon, not a namespace called nothing.
        assert_eq!(
            parts("10.0.0.1:7000/"),
            ("10.0.0.1".to_string(), 7000, ROOT_SUB_VAULT.to_string())
        );
    }

    #[test]
    fn an_ipv6_address_is_read_and_written_without_coming_apart() {
        let bare = VaultAddress::parse("::1").unwrap();
        assert_eq!(bare.address(), "::1");
        assert_eq!(bare.link().written_host(), "[::1]");

        // The colons of an ipv6 address would read as the port's, so it is bracketed when a
        // port follows it — including where the port is left out.
        assert_eq!(bare.to_string(), "rola://[::1]/");

        let with_port = VaultAddress::parse("rola://[::1]:7000/sub").unwrap();
        assert_eq!(with_port.address(), "::1");
        assert_eq!(with_port.sub(), "sub");
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
            "alice@vault.example.com:/srv/vaults/alpha",
            "alice@vault.example.com:2222:/srv/vaults/alpha",
            "alice@vault.example.com:project/alpha",
            "alice@[::1]:2222:/srv/vaults/alpha",
            "alice@[::1]:/srv/vaults/alpha",
            "alice@host:/srv/a vault's path",
        ] {
            let address = VaultAddress::parse(written).unwrap();
            let read_back = VaultAddress::parse(&address.to_string()).unwrap();

            assert_eq!(read_back, address, "{written}");
            assert_eq!(read_back.to_string(), address.to_string(), "{written}");
        }
    }

    #[test]
    fn what_was_written_is_kept_as_it_was_written() {
        let address = VaultAddress::parse("10.0.0.1:7000").unwrap();

        // `raw` is the caller's own words, not the link they add up to...
        assert_eq!(address.raw(), "10.0.0.1:7000");
        assert_eq!(address.to_string(), "rola://10.0.0.1:7000/");

        // ...and an SSH address keeps its own words too, which the link writes differently only
        // when it has to (here it does not).
        let ssh = VaultAddress::parse("alice@host:2222:/srv/alpha").unwrap();
        assert_eq!(ssh.raw(), "alice@host:2222:/srv/alpha");
        assert_eq!(ssh.to_string(), "alice@host:2222:/srv/alpha");
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
        assert_eq!(vault.link().written_host(), "10.0.0.1");
        assert_eq!(vault.sub(), ROOT_SUB_VAULT);
    }

    #[test]
    fn the_abi_reads_back_the_parts_the_link_was_written_with() {
        let address = VaultAddress::parse("rola://10.0.0.1:7000/vaults/alpha").unwrap();

        assert_eq!(address.read_raw(), "rola://10.0.0.1:7000/vaults/alpha");
        assert_eq!(address.read_addr(), "10.0.0.1");
        assert_eq!(address.read_port(), 7000);
        assert_eq!(address.read_kind(), 0);
        assert_eq!(address.read_ssh_user(), "");
        assert_eq!(address.read_ssh_path(), "");

        // An SSH address is told apart by the same three reads, and a port it did not name is
        // the zero no session is made to.
        let ssh = VaultAddress::parse("alice@host:/srv/alpha").unwrap();
        assert_eq!(ssh.read_addr(), "host");
        assert_eq!(ssh.read_port(), 0);
        assert_eq!(ssh.read_kind(), 1);
        assert_eq!(ssh.read_ssh_user(), "alice");
        assert_eq!(ssh.read_ssh_path(), "/srv/alpha");

        let ported = VaultAddress::parse("alice@host:2222:/srv/alpha").unwrap();
        assert_eq!(ported.read_port(), 2222);
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
    fn a_daemon_link_is_dialled_at_its_host_and_port() {
        // Which Vault under the daemon is meant is not part of where to knock, so what is dialled
        // is the host and the port and nothing else.
        let root = VaultAddress::parse("10.0.0.1").unwrap();
        let (host, port) = match root.link() {
            Link::Daemon { host, port } => (host.clone(), *port),
            Link::Ssh { .. } => panic!("a bare address names a daemon"),
        };
        assert_eq!(
            format!("{}:{port}", root.link().written_host()),
            "10.0.0.1:7717"
        );
        assert_eq!((host.as_str(), port), ("10.0.0.1", VAULT_DEFAULT_PORT));

        // An ipv6 address is bracketed, so the port after it reads as the port.
        let v6 = VaultAddress::parse("rola://[::1]:7000/sub").unwrap();
        assert_eq!(v6.link().written_host(), "[::1]");

        // A name is written as it is: there is no colon in it to be taken for the port's.
        let named = VaultAddress::parse("rola://example.com/sub").unwrap();
        assert_eq!(named.link().written_host(), "example.com");
    }

    #[test]
    fn an_account_and_a_path_are_read_as_an_ssh_session() {
        // The four spellings an SSH address has: with and without a port, absolute and relative,
        // and an ipv6 host beside them all.
        assert_eq!(
            ssh("alice@vault.example.com:/srv/vaults/alpha"),
            (
                "alice".to_string(),
                "vault.example.com".to_string(),
                None,
                "/srv/vaults/alpha".to_string()
            )
        );
        assert_eq!(
            ssh("alice@vault.example.com:2222:/srv/vaults/alpha"),
            (
                "alice".to_string(),
                "vault.example.com".to_string(),
                Some(2222),
                "/srv/vaults/alpha".to_string()
            )
        );
        assert_eq!(
            ssh("alice@vault.example.com:project/alpha"),
            (
                "alice".to_string(),
                "vault.example.com".to_string(),
                None,
                "project/alpha".to_string()
            )
        );
        assert_eq!(
            ssh("alice@[::1]:2222:/srv/vaults/alpha"),
            (
                "alice".to_string(),
                "::1".to_string(),
                Some(2222),
                "/srv/vaults/alpha".to_string()
            )
        );

        // An SSH address names no namespace under a daemon, and is not dialled over TCP: the
        // session is a process, not an address to connect to.
        let address = VaultAddress::parse("alice@host:/srv/alpha").unwrap();
        assert_eq!(address.sub(), ROOT_SUB_VAULT);
        assert_eq!(address.address(), "host");
        assert!(matches!(address.link(), Link::Ssh { .. }));
    }

    #[test]
    fn what_would_make_an_ssh_address_ambiguous_is_refused() {
        for written in [
            // No account, and no host.
            "@host:/srv/alpha",
            "alice@:/srv/alpha",
            "alice@",
            // No path.
            "alice@host:",
            "alice@host:2222:",
            // A port no session is made to, and one no machine has.
            "alice@host:0:/srv/alpha",
            "alice@host:65536:/srv/alpha",
            "alice@host:70000:/srv/alpha",
            // Two colons after the host: nothing says which of them is the port.
            "alice@host:a:b:/srv/alpha",
            "alice@host:/srv/alpha:beta",
            // An account a session is not started as, and a host that is not one.
            "ali ce@host:/srv/alpha",
            "alice@not a host:/srv/alpha",
            "alice@host/alpha:/srv/alpha",
            // A path the far side would read as a flag.
            "alice@host:-oProxyCommand=evil",
            // A path that would end the line ssh is handed.
            "alice@host:/srv/alpha\nrm -rf /",
            // The scheme is a daemon link and nothing else.
            "rola://alice@host:/srv/alpha",
        ] {
            assert!(VaultAddress::parse(written).is_err(), "{written}");
        }
    }

    #[test]
    fn an_ssh_address_is_written_as_it_was_read() {
        for (written, shown) in [
            ("alice@host:/srv/alpha", "alice@host:/srv/alpha"),
            ("alice@host:2222:/srv/alpha", "alice@host:2222:/srv/alpha"),
            ("alice@host:alpha", "alice@host:alpha"),
            ("alice@[::1]:2222:/srv/alpha", "alice@[::1]:2222:/srv/alpha"),
            ("alice@[::1]:/srv/alpha", "alice@[::1]:/srv/alpha"),
        ] {
            assert_eq!(VaultAddress::parse(written).unwrap().to_string(), shown);
        }

        // A path is written back exactly as it was given, spaces and quotes and all: what it
        // names is the far side's to read, and nothing here escapes it.
        let awkward = "alice@host:/srv/a vault's \"path\"";
        assert_eq!(VaultAddress::parse(awkward).unwrap().to_string(), awkward);
    }

    #[test]
    fn an_ssh_address_is_one_place_only_with_the_port_it_names() {
        assert_eq!(
            VaultAddress::parse("alice@host:/srv/alpha").unwrap(),
            VaultAddress::parse("alice@host:/srv/alpha").unwrap()
        );

        // A port that was written is part of where the session goes.
        assert_ne!(
            VaultAddress::parse("alice@host:/srv/alpha").unwrap(),
            VaultAddress::parse("alice@host:2222:/srv/alpha").unwrap()
        );

        // So is the path, and so is the account.
        assert_ne!(
            VaultAddress::parse("alice@host:/srv/alpha").unwrap(),
            VaultAddress::parse("alice@host:/srv/beta").unwrap()
        );
        assert_ne!(
            VaultAddress::parse("alice@host:/srv/alpha").unwrap(),
            VaultAddress::parse("bob@host:/srv/alpha").unwrap()
        );

        // An SSH session and a daemon are never the same place, whatever their hosts say.
        assert_ne!(
            VaultAddress::parse("alice@host:/srv/alpha").unwrap(),
            VaultAddress::parse("host").unwrap()
        );
    }

    #[test]
    fn the_spellings_that_were_read_as_a_daemon_still_are() {
        // Nothing written before SSH existed changes reading: none of them carry an account, so
        // none of them can be read as a session.
        for (written, host, port, sub) in [
            ("10.0.0.1", "10.0.0.1", VAULT_DEFAULT_PORT, ROOT_SUB_VAULT),
            ("7717", "127.0.0.1", 7717, ROOT_SUB_VAULT),
            ("localhost:7717", "localhost", 7717, ROOT_SUB_VAULT),
            ("10.0.0.1:7717/alpha", "10.0.0.1", 7717, "alpha"),
            (
                "10.0.0.1/alpha/vaults/beta",
                "10.0.0.1",
                VAULT_DEFAULT_PORT,
                "alpha/vaults/beta",
            ),
            (
                "rola://10.0.0.1/alpha",
                "10.0.0.1",
                VAULT_DEFAULT_PORT,
                "alpha",
            ),
            ("::1", "::1", VAULT_DEFAULT_PORT, ROOT_SUB_VAULT),
            ("[::1]:7717", "::1", 7717, ROOT_SUB_VAULT),
        ] {
            assert_eq!(
                parts(written),
                (host.to_string(), port, sub.to_string()),
                "{written}"
            );
        }
    }
}
