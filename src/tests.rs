use super::{crate_version, readiness_message};

#[test]
fn version_matches_package_metadata() {
  assert_eq!(crate_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn readiness_message_mentions_reservation() {
  assert!(readiness_message().contains("reserves"));
  assert!(readiness_message().contains("fraktor"));
}
