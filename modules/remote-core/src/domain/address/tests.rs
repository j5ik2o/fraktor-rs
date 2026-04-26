use alloc::{format, string::ToString};

use crate::domain::address::{ActorPathScheme, Address, RemoteNodeId, UniqueAddress};

#[test]
fn address_accessors_return_construction_values() {
  let addr = Address::new("user-system", "127.0.0.1", 6001);
  assert_eq!(addr.system(), "user-system");
  assert_eq!(addr.host(), "127.0.0.1");
  assert_eq!(addr.port(), 6001);
}

#[test]
fn address_equality_compares_all_fields() {
  let a = Address::new("sys", "host", 1);
  let b = Address::new("sys", "host", 1);
  let c = Address::new("sys", "host", 2);
  let d = Address::new("other", "host", 1);
  assert_eq!(a, b);
  assert_ne!(a, c);
  assert_ne!(a, d);
}

#[test]
fn address_display_uses_pekko_like_format() {
  let addr = Address::new("sys", "host", 2552);
  assert_eq!(addr.to_string(), "sys@host:2552");
}

#[test]
fn unique_address_accessors() {
  let base = Address::new("sys", "host", 10);
  let uniq = UniqueAddress::new(base.clone(), 42);
  assert_eq!(uniq.address(), &base);
  assert_eq!(uniq.uid(), 42);
}

#[test]
fn unique_address_equality_includes_uid() {
  let base = Address::new("sys", "host", 10);
  let a = UniqueAddress::new(base.clone(), 1);
  let b = UniqueAddress::new(base.clone(), 1);
  let c = UniqueAddress::new(base, 2);
  assert_eq!(a, b);
  assert_ne!(a, c);
}

#[test]
fn unique_address_display_includes_uid() {
  let base = Address::new("sys", "host", 7);
  let uniq = UniqueAddress::new(base, 99);
  assert_eq!(uniq.to_string(), "sys@host:7#99");
}

#[test]
fn unique_address_supports_zero_uid_sentinel() {
  let base = Address::new("sys", "host", 1);
  let unconfirmed = UniqueAddress::new(base.clone(), 0);
  let confirmed = UniqueAddress::new(base, 17);
  assert_ne!(unconfirmed, confirmed);
  assert_eq!(unconfirmed.uid(), 0);
}

#[test]
fn remote_node_id_accessors_and_optional_port() {
  let node = RemoteNodeId::new("sys", "host", Some(1234), 7);
  assert_eq!(node.system(), "sys");
  assert_eq!(node.host(), "host");
  assert_eq!(node.port(), Some(1234));
  assert_eq!(node.uid(), 7);

  let no_port = RemoteNodeId::new("sys", "host", None, 0);
  assert!(no_port.port().is_none());
}

#[test]
fn actor_path_scheme_has_expected_str() {
  assert_eq!(ActorPathScheme::Fraktor.as_str(), "fraktor");
  assert_eq!(ActorPathScheme::FraktorTcp.as_str(), "fraktor.tcp");
}

#[test]
fn address_format_via_write_macro() {
  let addr = Address::new("s", "h", 0);
  let s = format!("{}", addr);
  assert_eq!(s, "s@h:0");
}
