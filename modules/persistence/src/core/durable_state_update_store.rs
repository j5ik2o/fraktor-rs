//! Durable state update store abstraction.

use crate::core::{
  durable_state_store::{DurableStateStore, DurableStateStoreFuture},
};

/// Durable state store extension that exposes update notifications.
pub trait DurableStateUpdateStore<A: Send>: DurableStateStore<A> {
  /// Loads the next update after `from_offset` for the persistence identifier.
  ///
  /// Returns `Some((next_offset, value))` when a new update exists, otherwise `None`.
  fn changes<'a>(
    &'a self,
    persistence_id: &'a str,
    from_offset: usize,
  ) -> DurableStateStoreFuture<'a, Option<(usize, A)>>;
}
