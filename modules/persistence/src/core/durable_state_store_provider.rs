//! Durable state store provider abstraction.

use alloc::boxed::Box;

use crate::core::durable_state_store::DurableStateStore;

/// Factory trait that creates durable state store instances.
pub trait DurableStateStoreProvider<A: Send>: Send + Sync + 'static {
  /// Creates a durable state store instance.
  fn durable_state_store(&self) -> Box<dyn DurableStateStore<A>>;
}
