//! One question about the network, asked because two tiers need the answer.
//!
//! Not a networking module -- this crate measures what the machine is, and
//! "the address another machine can reach me on" is one of those facts. It
//! lives here rather than in the server because the CLI needs it too: a
//! headless node prints its own route as a QR code, and it must be the same
//! route the server would have served.

/// The address on this machine another machine can reach, and whether it
/// turned out to be loopback after all.
///
/// When `host` names a concrete interface, that is the answer and there is
/// nothing to discover. When it is a wildcard -- `0.0.0.0`, which is the whole
/// reason anyone passes `--host` -- the socket is bound to every interface at
/// once and cannot say which one a peer will arrive on.
///
/// **The kernel can.** Opening a UDP socket and *connecting* it performs a
/// route lookup and fills in the local address, and connecting a UDP socket
/// transmits nothing. The destination is TEST-NET-1 from RFC 5737, an address
/// reserved for documentation that is guaranteed never to be a real host --
/// chosen so that no packet could reach anyone even if one were sent. This is
/// the whole reason the function is not "read the interface list": there is no
/// portable interface list in `std`, and the one that matters is not the first
/// one anyway, it is the one the default route points out of.
///
/// Falls back to loopback with `true`, which is honest rather than convenient:
/// on a machine with no route out, loopback genuinely is all there is, and a
/// caller that prints a route needs to say so instead of handing out an
/// address nothing can reach.
pub fn reachable_address(host: &str) -> (String, bool) {
    let wildcard = host == "0.0.0.0" || host == "::" || host == "[::]" || host.is_empty();
    if !wildcard {
        let loopback =
            host == "127.0.0.1" || host == "localhost" || host == "::1" || host == "[::1]";
        return (host.to_string(), loopback);
    }
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("192.0.2.1:9").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                let ip = addr.ip();
                if !ip.is_loopback() && !ip.is_unspecified() {
                    return (ip.to_string(), false);
                }
            }
        }
    }
    ("127.0.0.1".to_string(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_host_is_returned_unchanged() {
        assert_eq!(
            reachable_address("10.1.2.3"),
            ("10.1.2.3".to_string(), false)
        );
        assert_eq!(
            reachable_address("127.0.0.1"),
            ("127.0.0.1".to_string(), true)
        );
        assert_eq!(
            reachable_address("localhost"),
            ("localhost".to_string(), true)
        );
    }

    /// A wildcard must resolve to *something* parseable, and it must never
    /// hand back the wildcard itself -- `http://0.0.0.0:8080` in a QR code is
    /// a route to nowhere that looks exactly like a route.
    #[test]
    fn a_wildcard_resolves_to_a_real_address() {
        for host in ["0.0.0.0", "::", ""] {
            let (addr, loopback) = reachable_address(host);
            assert!(
                addr.parse::<std::net::IpAddr>().is_ok(),
                "{addr} for {host}"
            );
            assert_ne!(addr, "0.0.0.0");
            // Whatever it found, `loopback` has to agree with the address.
            let ip: std::net::IpAddr = addr.parse().unwrap();
            assert_eq!(
                loopback,
                ip.is_loopback(),
                "{addr} reported loopback={loopback}"
            );
        }
    }
}
