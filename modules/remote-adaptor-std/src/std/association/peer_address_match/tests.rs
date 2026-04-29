use fraktor_remote_core_rs::core::address::Address;

use super::peer_matches_address;

#[test]
fn matches_ipv4_socket_addr_with_address() {
  let address = Address::new("remote-sys", "10.0.0.1", 2552);

  assert!(peer_matches_address("10.0.0.1:2552", &address));
}

#[test]
fn matches_ipv6_bracketed_socket_addr_with_address() {
  let address = Address::new("remote-sys", "::1", 2552);

  assert!(peer_matches_address("[::1]:2552", &address));
}

#[test]
fn does_not_match_when_host_differs() {
  let address = Address::new("remote-sys", "10.0.0.1", 2552);

  assert!(!peer_matches_address("10.0.0.2:2552", &address));
}

#[test]
fn does_not_match_when_port_differs() {
  let address = Address::new("remote-sys", "10.0.0.1", 2552);

  assert!(!peer_matches_address("10.0.0.1:2553", &address));
}

#[test]
fn does_not_match_when_peer_lacks_port() {
  let address = Address::new("remote-sys", "10.0.0.1", 2552);

  assert!(!peer_matches_address("10.0.0.1", &address));
}
