//! Addressing the daemon an action is spoken to.

use std::net::{IpAddr, SocketAddr};

use rorolala_errors::AddrError;

/// The address a target names.
///
/// An ip and a port is an address, and so is an ip alone: the port that was left out means
/// `default_port`, which is the one a Vault listens on when its configuration names no other.
/// A name is not an address — it has to be resolved before it can be written down — so what is
/// neither is refused rather than guessed at.
///
/// # Errors
///
/// Returns an [`AddrError`] naming the target when it is neither an ip nor an ip and a port.
pub fn parse_address(target: &str, default_port: u16) -> Result<SocketAddr, AddrError> {
    if let Ok(address) = target.parse::<SocketAddr>() {
        return Ok(address);
    }

    if let Ok(ip) = target.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }

    Err(AddrError::new(target.to_string()))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::parse_address;

    #[test]
    fn an_ip_and_a_port_is_taken_as_it_is() {
        assert_eq!(
            parse_address("127.0.0.1:7000", 7717).unwrap(),
            "127.0.0.1:7000".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn an_ip_alone_takes_the_default_port() {
        assert_eq!(
            parse_address("127.0.0.1", 7717).unwrap(),
            "127.0.0.1:7717".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            parse_address("::1", 7717).unwrap(),
            "[::1]:7717".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn what_is_not_an_address_says_which() {
        let error = parse_address("nowhere", 7717).unwrap_err();

        assert_eq!(error.target(), "nowhere");
    }
}
