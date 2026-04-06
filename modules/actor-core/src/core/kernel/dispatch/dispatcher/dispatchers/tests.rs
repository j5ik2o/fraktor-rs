use core::time::Duration;

use super::*;
use crate::core::kernel::dispatch::dispatcher::{
  DispatcherRegistryEntry, DispatcherSettings, InlineDispatcherProvider,
};

#[test]
fn register_and_resolve_dispatcher_by_id() {
  let mut registry = Dispatchers::new();
  registry.ensure_default();

  let entry = DispatcherRegistryEntry::new(
    InlineDispatcherProvider::new(),
    DispatcherSettings::default().with_starvation_deadline(Some(Duration::from_millis(5))),
  );
  registry.register("custom", entry).expect("register dispatcher");

  let resolved = registry.resolve("custom").expect("resolve dispatcher");
  assert_eq!(resolved.settings().starvation_deadline(), Some(Duration::from_millis(5)));
}

#[test]
fn register_duplicate_dispatcher_fails() {
  let mut registry = Dispatchers::new();
  registry.ensure_default();
  let entry = DispatcherRegistryEntry::new(InlineDispatcherProvider::new(), DispatcherSettings::default());
  registry.register("dup", entry.clone()).expect("first register");
  assert!(matches!(registry.register("dup", entry), Err(DispatcherRegistryError::Duplicate(_))));
}

#[test]
fn ensure_default_makes_default_id_available() {
  let mut registry = Dispatchers::new();
  registry.ensure_default();
  assert!(registry.resolve(DEFAULT_DISPATCHER_ID).is_ok());
  assert!(registry.resolve(DEFAULT_BLOCKING_DISPATCHER_ID).is_ok());
}
