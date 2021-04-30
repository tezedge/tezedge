use std::net::{IpAddr, SocketAddr};

pub trait DnsService {
    /// Try to resolve common peer name into Socket Address representation.
    fn resolve_dns_name_to_peer_address(
        &mut self,
        address: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, dns_lookup::LookupError>;
}

#[derive(Debug, Default, Clone)]
pub struct DnsServiceDefault;

impl DnsServiceDefault {
    pub fn new() -> Self {
        Self {}
    }
}

impl DnsService for DnsServiceDefault {
    fn resolve_dns_name_to_peer_address(
        &mut self,
        address: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, dns_lookup::LookupError> {
        // filter just for [`AI_SOCKTYPE SOCK_STREAM`]
        let hints = dns_lookup::AddrInfoHints {
            socktype: i32::from(dns_lookup::SockType::Stream),
            ..dns_lookup::AddrInfoHints::default()
        };

        let addrs =
            dns_lookup::getaddrinfo(Some(address), Some(port.to_string().as_str()), Some(hints))?
                .filter(Result::is_ok)
                .map(Result::unwrap)
                .filter(|info: &dns_lookup::AddrInfo| {
                    // filter just IP_NET and IP_NET6 addresses
                    dns_lookup::AddrFamily::Inet.eq(&info.address)
                        || dns_lookup::AddrFamily::Inet6.eq(&info.address)
                })
                .map(|info: dns_lookup::AddrInfo| {
                    // convert to uniform IPv6 format
                    match &info.sockaddr {
                        SocketAddr::V4(ipv4) => {
                            // convert ipv4 to ipv6
                            SocketAddr::new(IpAddr::V6(ipv4.ip().to_ipv6_mapped()), ipv4.port())
                        }
                        SocketAddr::V6(_) => info.sockaddr,
                    }
                })
                .collect();
        Ok(addrs)
    }
}
