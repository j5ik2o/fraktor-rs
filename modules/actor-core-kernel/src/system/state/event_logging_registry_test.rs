use super::EventLoggingRegistry;

#[test]
fn event_logging_registry_starts_with_empty_dead_letters() {
  let registry = EventLoggingRegistry::with_capacities(4, 4);

  assert!(registry.dead_letter.entries().is_empty());
  let _event_stream = registry.event_stream.clone();
}
