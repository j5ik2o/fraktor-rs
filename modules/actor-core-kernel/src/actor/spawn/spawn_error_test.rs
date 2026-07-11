extern crate alloc;

use alloc::{format, string::String};
use core::error::Error;

use super::SpawnError;

#[test]
fn spawn_error_name_conflict() {
  let error = SpawnError::name_conflict("test-actor");
  match error {
    | SpawnError::NameConflict(name) => assert_eq!(name, "test-actor"),
    | _ => panic!("Expected NameConflict"),
  }
}

#[test]
fn spawn_error_system_unavailable() {
  let error = SpawnError::system_unavailable();
  match error {
    | SpawnError::SystemUnavailable => {},
    | _ => panic!("Expected SystemUnavailable"),
  }
}

#[test]
fn spawn_error_invalid_props() {
  let error = SpawnError::invalid_props("test reason");
  match error {
    | SpawnError::InvalidProps(reason) => assert_eq!(reason, "test reason"),
    | _ => panic!("Expected InvalidProps"),
  }
}

#[test]
fn spawn_error_debug() {
  let error1 = SpawnError::name_conflict("test");
  let error2 = SpawnError::system_unavailable();
  let error3 = SpawnError::invalid_props("reason");

  assert!(!format!("{:?}", error1).is_empty());
  assert!(!format!("{:?}", error2).is_empty());
  assert!(!format!("{:?}", error3).is_empty());
}

#[test]
fn spawn_error_display_describes_failure() {
  assert_eq!(format!("{}", SpawnError::name_conflict("test")), "actor name conflict: test");
  assert_eq!(format!("{}", SpawnError::system_unavailable()), "actor system unavailable");
  assert_eq!(format!("{}", SpawnError::invalid_props("reason")), "invalid actor props: reason");
  assert_eq!(format!("{}", SpawnError::system_not_bootstrapped()), "actor system not bootstrapped");
  assert_eq!(format!("{}", SpawnError::DispatcherAlreadyOwned), "pinned dispatcher already owned");
  assert_eq!(format!("{}", SpawnError::SystemBuildError(String::from("boom"))), "actor system build failed: boom");
}

#[test]
fn spawn_error_implements_core_error() {
  fn assert_error<E: Error>() {}

  assert_error::<SpawnError>();
}
