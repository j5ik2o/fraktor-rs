#![cfg(test)]

use super::NameRegistryError;

#[test]
fn duplicate_carries_name() {
  let error = NameRegistryError::Duplicate("worker".into());
  match error {
    | NameRegistryError::Duplicate(name) => assert_eq!(name, "worker"),
  }
}
