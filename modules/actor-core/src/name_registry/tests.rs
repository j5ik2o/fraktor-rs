#![cfg(test)]

use super::NameRegistry;
use crate::{name_registry_error::NameRegistryError, Pid};

#[test]
fn register_and_resolve_name() {
  let mut registry = NameRegistry::new();
  let pid = Pid::new(1, 0);
  assert!(registry.register("worker", pid).is_ok());
  assert_eq!(registry.resolve("worker"), Some(pid));
}

#[test]
fn duplicate_registration_fails() {
  let mut registry = NameRegistry::new();
  let pid = Pid::new(2, 0);
  registry.register("worker", pid).unwrap();
  let error = registry.register("worker", pid).unwrap_err();
  assert!(matches!(error, NameRegistryError::Duplicate(name) if name == "worker"));
}

#[test]
fn generate_anonymous_uses_pid() {
  let registry = NameRegistry::new();
  let pid = Pid::new(7, 3);
  assert_eq!(registry.generate_anonymous(pid), "anon-7:3");
}
