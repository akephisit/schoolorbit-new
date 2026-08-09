use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;
use ipnet::IpNet;

use crate::modules::auth::session_crypto::normalize_ip;

pub fn client_address(
    peer: SocketAddr,
    headers: &HeaderMap,
    trusted_proxy_cidrs: &[IpNet],
) -> IpAddr {
    let direct = normalize_ip(peer.ip());
    if !trusted_proxy_cidrs
        .iter()
        .any(|network| network.contains(&direct))
    {
        return direct;
    }

    let mut values = headers.get_all("x-real-ip").iter();
    let Some(value) = values.next() else {
        return direct;
    };
    if values.next().is_some() {
        return direct;
    }

    let Ok(value) = value.to_str() else {
        return direct;
    };
    if value.is_empty() || value.trim() != value {
        return direct;
    }

    value.parse::<IpAddr>().map(normalize_ip).unwrap_or(direct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderName, HeaderValue};
    use std::net::{IpAddr, SocketAddr};

    fn peer(value: &str) -> SocketAddr {
        value.parse().unwrap()
    }

    #[test]
    fn forwarded_address_is_used_only_for_a_trusted_peer() {
        let headers = HeaderMap::from_iter([(
            HeaderName::from_static("x-real-ip"),
            HeaderValue::from_static("203.0.113.9"),
        )]);
        let trusted = vec!["10.0.0.0/8".parse().unwrap()];

        assert_eq!(
            client_address(peer("10.88.0.4:41234"), &headers, &trusted),
            "203.0.113.9".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            client_address(peer("198.51.100.7:41234"), &headers, &trusted),
            "198.51.100.7".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn duplicate_malformed_or_non_bare_real_ip_falls_back_to_peer() {
        let trusted = vec!["10.0.0.0/8".parse().unwrap()];
        let direct = "10.88.0.4".parse::<IpAddr>().unwrap();

        let mut duplicate = HeaderMap::new();
        duplicate.append("x-real-ip", HeaderValue::from_static("203.0.113.9"));
        duplicate.append("x-real-ip", HeaderValue::from_static("203.0.113.10"));
        assert_eq!(
            client_address(peer("10.88.0.4:1"), &duplicate, &trusted),
            direct
        );

        for value in ["203.0.113.9, 10.0.0.1", "203.0.113.9:443", " 203.0.113.9"] {
            let headers = HeaderMap::from_iter([(
                HeaderName::from_static("x-real-ip"),
                HeaderValue::from_str(value).unwrap(),
            )]);
            assert_eq!(
                client_address(peer("10.88.0.4:1"), &headers, &trusted),
                direct
            );
        }
    }

    #[test]
    fn x_forwarded_for_is_never_an_identity_source() {
        let headers = HeaderMap::from_iter([(
            HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_static("203.0.113.9"),
        )]);
        let trusted = vec!["10.0.0.0/8".parse().unwrap()];

        assert_eq!(
            client_address(peer("10.88.0.4:1"), &headers, &trusted),
            "10.88.0.4".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn ipv4_mapped_ipv6_is_normalized() {
        let headers = HeaderMap::new();
        let trusted = Vec::new();

        assert_eq!(
            client_address(peer("[::ffff:203.0.113.9]:443"), &headers, &trusted),
            "203.0.113.9".parse::<IpAddr>().unwrap()
        );
    }
}
