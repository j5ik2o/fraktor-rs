use super::Address;
use crate::core::kernel::actor::actor_path::ActorPathParts;

#[test]
fn local_address_has_no_host_or_port() {
  let addr = Address::local("test-system");
  assert_eq!(addr.system(), "test-system");
  assert_eq!(addr.protocol(), "fraktor");
  assert!(addr.host().is_none());
  assert!(addr.port().is_none());
  assert!(addr.has_local_scope());
  assert!(!addr.has_global_scope());
}

#[test]
fn remote_address_has_host_and_port() {
  let addr = Address::remote("test-system", "127.0.0.1", 2552);
  assert_eq!(addr.system(), "test-system");
  assert_eq!(addr.protocol(), "fraktor.tcp");
  assert_eq!(addr.host(), Some("127.0.0.1"));
  assert_eq!(addr.port(), Some(2552));
  assert!(addr.has_global_scope());
  assert!(!addr.has_local_scope());
}

#[test]
fn new_creates_local_address_with_custom_protocol() {
  let addr = Address::new("custom", "my-system");
  assert_eq!(addr.protocol(), "custom");
  assert_eq!(addr.system(), "my-system");
  assert!(addr.has_local_scope());
}

#[test]
fn new_remote_creates_address_with_custom_protocol() {
  let addr = Address::new_remote("pekko", "my-system", "10.0.0.1", 9000);
  assert_eq!(addr.protocol(), "pekko");
  assert_eq!(addr.system(), "my-system");
  assert_eq!(addr.host(), Some("10.0.0.1"));
  assert_eq!(addr.port(), Some(9000));
  assert!(addr.has_global_scope());
}

#[test]
fn from_parts_local() {
  let parts = ActorPathParts::local("my-system");
  let addr = Address::from_parts(&parts);
  assert_eq!(addr.system(), "my-system");
  assert_eq!(addr.protocol(), "fraktor");
  assert!(addr.has_local_scope());
}

#[test]
fn from_parts_remote() {
  let parts = ActorPathParts::with_authority("my-system", Some(("10.0.0.1", 9000)));
  let addr = Address::from_parts(&parts);
  assert_eq!(addr.system(), "my-system");
  assert_eq!(addr.protocol(), "fraktor.tcp");
  assert_eq!(addr.host(), Some("10.0.0.1"));
  assert_eq!(addr.port(), Some(9000));
}

#[test]
fn to_uri_string_local() {
  let addr = Address::local("sys");
  assert_eq!(addr.to_uri_string(), "fraktor://sys");
}

#[test]
fn to_uri_string_remote() {
  let addr = Address::remote("sys", "host", 2552);
  assert_eq!(addr.to_uri_string(), "fraktor.tcp://sys@host:2552");
}

#[test]
fn display_matches_uri_string() {
  extern crate alloc;
  use alloc::format;

  let addr = Address::remote("sys", "host", 2552);
  assert_eq!(format!("{}", addr), addr.to_uri_string());
}

#[test]
fn address_equality() {
  let a = Address::local("sys");
  let b = Address::local("sys");
  assert_eq!(a, b);
}

#[test]
fn address_clone() {
  let a = Address::remote("sys", "host", 2552);
  let b = a.clone();
  assert_eq!(a, b);
}

#[test]
fn host_port_for_remote_address() {
  let addr = Address::remote("sys", "host", 2552);
  assert_eq!(addr.host_port(), "sys@host:2552");
}

#[test]
fn host_port_for_local_address() {
  let addr = Address::local("sys");
  assert_eq!(addr.host_port(), "sys");
}

#[test]
fn host_port_for_custom_protocol() {
  let addr = Address::new_remote("pekko", "sys", "10.0.0.1", 25520);
  assert_eq!(addr.host_port(), "sys@10.0.0.1:25520");
  assert_eq!(addr.to_uri_string(), "pekko://sys@10.0.0.1:25520");
}
