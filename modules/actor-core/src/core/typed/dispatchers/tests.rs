use crate::core::{
  kernel::dispatch::dispatcher::DispatchersError,
  typed::{DispatcherSelector, dispatchers::Dispatchers},
};

// --- helpers ---------------------------------------------------------------

fn new_dispatchers_with_defaults() -> Dispatchers {
  use crate::core::kernel::system::ActorSystem;
  let system = ActorSystem::new_empty();
  Dispatchers::new(system.state())
}

// --- lookup: Default selector ----------------------------------------------

#[test]
fn lookup_default_selector_resolves_default_dispatcher() {
  // Given: a Dispatchers facade backed by a system with default dispatchers
  let dispatchers = new_dispatchers_with_defaults();

  // When: lookup is called with DispatcherSelector::Default
  let result = dispatchers.lookup(&DispatcherSelector::Default);

  // Then: a valid dispatcher handle is returned
  assert!(result.is_ok(), "Default selector should resolve to the default dispatcher");
}

// --- lookup: FromConfig selector -------------------------------------------

#[test]
fn lookup_from_config_selector_resolves_registered_dispatcher() {
  let dispatchers = new_dispatchers_with_defaults();

  let selector = DispatcherSelector::from_config(Dispatchers::DEFAULT_DISPATCHER_ID);
  let result = dispatchers.lookup(&selector);

  assert!(result.is_ok(), "FromConfig(DefaultDispatcherId) should resolve to the default dispatcher");
}

#[test]
fn lookup_from_config_selector_normalizes_internal_dispatcher_id_to_default() {
  let dispatchers = new_dispatchers_with_defaults();

  let selector = DispatcherSelector::from_config(Dispatchers::INTERNAL_DISPATCHER_ID);
  let result = dispatchers.lookup(&selector);

  assert!(result.is_ok(), "FromConfig(InternalDispatcherId) should resolve to the default dispatcher");
}

#[test]
fn lookup_from_config_selector_returns_error_for_unknown_id() {
  let dispatchers = new_dispatchers_with_defaults();

  let selector = DispatcherSelector::from_config("non-existent-dispatcher");
  let result = dispatchers.lookup(&selector);

  assert!(
    matches!(result, Err(DispatchersError::Unknown(_))),
    "Unknown dispatcher id should return DispatchersError::Unknown"
  );
}

// --- lookup: SameAsParent selector -----------------------------------------

#[test]
fn lookup_same_as_parent_selector_falls_back_to_default() {
  let dispatchers = new_dispatchers_with_defaults();

  let result = dispatchers.lookup(&DispatcherSelector::SameAsParent);

  assert!(result.is_ok(), "SameAsParent should fall back to the default dispatcher in lookup");
}

// --- lookup: Blocking selector ---------------------------------------------

#[test]
fn lookup_blocking_selector_resolves_blocking_dispatcher() {
  let dispatchers = new_dispatchers_with_defaults();

  let result = dispatchers.lookup(&DispatcherSelector::Blocking);

  assert!(result.is_ok(), "Blocking selector should resolve to the blocking dispatcher");
}

// --- DEFAULT_DISPATCHER_ID constant ----------------------------------------

#[test]
fn default_dispatcher_id_matches_kernel_constant() {
  let id = Dispatchers::DEFAULT_DISPATCHER_ID;
  assert_eq!(id, "pekko.actor.default-dispatcher");
}

#[test]
fn internal_dispatcher_id_matches_pekko_constant() {
  let id = Dispatchers::INTERNAL_DISPATCHER_ID;
  assert_eq!(id, "pekko.actor.internal-dispatcher");
}

// --- shutdown --------------------------------------------------------------

#[test]
fn shutdown_is_callable_as_a_noop() {
  let dispatchers = new_dispatchers_with_defaults();

  dispatchers.shutdown();

  let result = dispatchers.lookup(&DispatcherSelector::Default);
  assert!(result.is_ok(), "shutdown() should not invalidate dispatcher lookup");
}

// --- lookup consistency: Default and SameAsParent resolve to same handle ----

#[test]
fn lookup_default_and_same_as_parent_resolve_to_equivalent_handle() {
  let dispatchers = new_dispatchers_with_defaults();

  let default_handle = dispatchers.lookup(&DispatcherSelector::Default).expect("Default");
  let same_as_parent_handle = dispatchers.lookup(&DispatcherSelector::SameAsParent).expect("SameAsParent");

  // Both selectors should target the same registered dispatcher id, so the
  // resolved handles report the same identifier and throughput.
  assert_eq!(default_handle.id(), same_as_parent_handle.id());
  assert_eq!(default_handle.throughput(), same_as_parent_handle.throughput());
}
