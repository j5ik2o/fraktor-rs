use core::time::Duration;

use super::*;

#[test]
fn register_and_resolve_dispatcher_by_id() {
  let registry = DispatchersGeneric::<NoStdToolbox>::new();
  registry.ensure_default();

  let config = DispatcherConfigGeneric::default().with_starvation_deadline(Some(Duration::from_millis(5)));
  registry.register("custom", config.clone()).expect("register dispatcher");

  let resolved = registry.resolve("custom").expect("resolve dispatcher");
  assert_eq!(resolved.starvation_deadline(), Some(Duration::from_millis(5)));
}

#[test]
fn register_duplicate_dispatcher_fails() {
  let registry = DispatchersGeneric::<NoStdToolbox>::new();
  registry.ensure_default();
  let config = DispatcherConfigGeneric::default();
  registry.register("dup", config.clone()).expect("first register");
  assert!(matches!(registry.register("dup", config), Err(DispatcherRegistryError::Duplicate(_))));
}

#[test]
fn ensure_default_makes_default_id_available() {
  let registry = DispatchersGeneric::<NoStdToolbox>::new();
  registry.ensure_default();
  assert!(registry.resolve(DEFAULT_DISPATCHER_ID).is_ok());
}
