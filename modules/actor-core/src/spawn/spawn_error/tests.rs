extern crate alloc;

use alloc::format;

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

  let _ = format!("{:?}", error1);
  let _ = format!("{:?}", error2);
  let _ = format!("{:?}", error3);
}
