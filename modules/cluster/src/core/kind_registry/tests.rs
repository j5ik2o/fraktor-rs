use alloc::string::String;

use super::{KindRegistry, TOPIC_ACTOR_KIND};
use crate::core::activated_kind::ActivatedKind;

#[test]
fn auto_registers_topic_kind_when_missing() {
  let mut registry = KindRegistry::new();
  let kinds = vec![ActivatedKind::new("worker")];

  registry.register_all(kinds);

  assert!(registry.contains(TOPIC_ACTOR_KIND));
  let names: Vec<_> = registry.all().into_iter().map(|k| k.name().to_string()).collect();
  assert!(names.contains(&String::from("worker")));
  assert!(names.contains(&String::from(TOPIC_ACTOR_KIND)));
}

#[test]
fn keeps_existing_topic_kind_without_duplication() {
  let mut registry = KindRegistry::new();
  let kinds = vec![ActivatedKind::new(TOPIC_ACTOR_KIND), ActivatedKind::new("analytics")];

  registry.register_all(kinds);

  let names: Vec<_> = registry.all().into_iter().map(|k| k.name().to_string()).collect();
  assert_eq!(2, names.len());
  assert!(names.contains(&String::from(TOPIC_ACTOR_KIND)));
  assert!(names.contains(&String::from("analytics")));
}

#[test]
fn records_invalid_kind_requests() {
  let mut registry = KindRegistry::new();
  registry.register_all(vec![ActivatedKind::new("worker")]);

  let missing = registry.get("unknown");
  assert!(missing.is_none());

  let recorded = registry.take_invalid_requests();
  assert_eq!(recorded, vec![String::from("unknown")]);
  assert!(registry.take_invalid_requests().is_empty());
}

#[test]
fn generation_increments_only_on_changes() {
  let mut registry = KindRegistry::new();
  assert_eq!(0, registry.generation());

  registry.register_all(vec![ActivatedKind::new("worker")]);
  assert_eq!(1, registry.generation());

  // same set should not change generation
  registry.register_all(vec![ActivatedKind::new("worker")]);
  assert_eq!(1, registry.generation());

  // adding new kind increments
  registry.register_all(vec![ActivatedKind::new("analytics")]);
  assert_eq!(2, registry.generation());
}
