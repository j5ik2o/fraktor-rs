use crate::core::{
  kernel::dispatch::dispatcher::DispatcherRegistryError,
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

  // Then: a valid DispatcherConfig is returned
  assert!(result.is_ok(), "Default selector should resolve to the default dispatcher");
}

// --- lookup: FromConfig selector -------------------------------------------

#[test]
fn lookup_from_config_selector_resolves_registered_dispatcher() {
  // Given: a Dispatchers facade backed by a system with default dispatchers
  let dispatchers = new_dispatchers_with_defaults();

  // When: lookup is called with Pekko's public default dispatcher id
  let selector = DispatcherSelector::from_config(Dispatchers::DEFAULT_DISPATCHER_ID);
  let result = dispatchers.lookup(&selector);

  // Then: the public id is normalized to the registered dispatcher
  assert!(result.is_ok(), "FromConfig(DefaultDispatcherId) should resolve to the default dispatcher");
}

#[test]
fn lookup_from_config_selector_normalizes_internal_dispatcher_id() {
  // Given: a Dispatchers facade backed by a system with default dispatchers
  let dispatchers = new_dispatchers_with_defaults();

  // When: lookup is called with Pekko's public internal dispatcher id
  let selector = DispatcherSelector::from_config(Dispatchers::INTERNAL_DISPATCHER_ID);
  let result = dispatchers.lookup(&selector);

  // Then: the public id is normalized to the registered dispatcher
  assert!(result.is_ok(), "FromConfig(InternalDispatcherId) should resolve to the default dispatcher");
}

#[test]
fn lookup_from_config_selector_returns_error_for_unknown_id() {
  // Given: a Dispatchers facade
  let dispatchers = new_dispatchers_with_defaults();

  // When: lookup is called with a non-existent dispatcher id
  let selector = DispatcherSelector::from_config("non-existent-dispatcher");
  let result = dispatchers.lookup(&selector);

  // Then: an Unknown error is returned
  assert!(
    matches!(result, Err(DispatcherRegistryError::Unknown(_))),
    "Unknown dispatcher id should return DispatcherRegistryError::Unknown"
  );
}

// --- lookup: SameAsParent selector -----------------------------------------

#[test]
fn lookup_same_as_parent_selector_falls_back_to_default() {
  // Given: a Dispatchers facade
  let dispatchers = new_dispatchers_with_defaults();

  // When: lookup is called with SameAsParent
  let result = dispatchers.lookup(&DispatcherSelector::SameAsParent);

  // Then: the default dispatcher is returned (SameAsParent is resolved at spawn time)
  assert!(result.is_ok(), "SameAsParent should fall back to the default dispatcher in lookup");
}

// --- lookup: Blocking selector ---------------------------------------------

#[test]
fn lookup_blocking_selector_resolves_blocking_dispatcher() {
  // Given: a Dispatchers facade with default dispatchers (including blocking)
  let dispatchers = new_dispatchers_with_defaults();

  // When: lookup is called with DispatcherSelector::Blocking
  let result = dispatchers.lookup(&DispatcherSelector::Blocking);

  // Then: the blocking dispatcher is returned
  assert!(result.is_ok(), "Blocking selector should resolve to the blocking dispatcher");
}

// --- DEFAULT_DISPATCHER_ID constant ----------------------------------------

#[test]
fn default_dispatcher_id_matches_kernel_constant() {
  // Given/When: the Dispatchers::DEFAULT_DISPATCHER_ID constant
  let id = Dispatchers::DEFAULT_DISPATCHER_ID;

  // Then: it matches Pekko's public default dispatcher id
  assert_eq!(id, "pekko.actor.default-dispatcher");
}

#[test]
fn internal_dispatcher_id_matches_pekko_constant() {
  // Given/When: the Dispatchers::INTERNAL_DISPATCHER_ID constant
  let id = Dispatchers::INTERNAL_DISPATCHER_ID;

  // Then: it matches Pekko's internal dispatcher id
  assert_eq!(id, "pekko.actor.internal-dispatcher");
}

// --- shutdown --------------------------------------------------------------

#[test]
fn shutdown_is_callable_as_a_noop() {
  // Given: a Dispatchers facade backed by a live actor system
  let dispatchers = new_dispatchers_with_defaults();

  // When: shutdown is called
  dispatchers.shutdown();

  // Then: the facade remains usable because shutdown is a safe no-op
  let result = dispatchers.lookup(&DispatcherSelector::Default);
  assert!(result.is_ok(), "shutdown() should not invalidate dispatcher lookup");
}

// --- lookup consistency: Default and SameAsParent resolve to same config ----

#[test]
fn lookup_default_and_same_as_parent_resolve_to_equivalent_config() {
  // Given: a Dispatchers facade
  let dispatchers = new_dispatchers_with_defaults();

  // When: both Default and SameAsParent are looked up
  let default_config = dispatchers.lookup(&DispatcherSelector::Default).expect("Default");
  let same_as_parent_config = dispatchers.lookup(&DispatcherSelector::SameAsParent).expect("SameAsParent");

  // Then: both resolve to the same configuration
  assert_eq!(
    default_config.starvation_deadline(),
    same_as_parent_config.starvation_deadline(),
    "Default and SameAsParent should resolve to equivalent configs"
  );
}
