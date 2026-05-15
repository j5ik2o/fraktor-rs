use super::NameRegistry;
use crate::actor::{Pid, spawn::NameRegistryError};

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
fn replace_if_updates_when_expected_pid_matches() {
  let mut registry = NameRegistry::new();
  let reserved = Pid::new(1, 0);
  let actual = Pid::new(2, 0);

  registry.register("worker", reserved).unwrap();

  assert!(registry.replace_if("worker", reserved, actual));
  assert_eq!(registry.resolve("worker"), Some(actual));
}

#[test]
fn replace_if_keeps_existing_pid_when_expected_pid_differs() {
  let mut registry = NameRegistry::new();
  let existing = Pid::new(1, 0);
  let actual = Pid::new(2, 0);

  registry.register("worker", existing).unwrap();

  assert!(!registry.replace_if("worker", Pid::new(9, 0), actual));
  assert_eq!(registry.resolve("worker"), Some(existing));
}

#[test]
fn generate_anonymous_uses_pid() {
  let registry = NameRegistry::new();
  let pid = Pid::new(7, 3);
  assert_eq!(registry.generate_anonymous(pid), "anon-7:3");
}
