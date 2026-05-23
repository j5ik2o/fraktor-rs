//! Durable state update store abstraction.

use crate::state::{DurableStateChange, DurableStateStore, DurableStateStoreFuture};

/// Durable state store extension that exposes update notifications.
pub trait DurableStateUpdateStore<A: Send>: DurableStateStore<A> {
  /// Loads the next tagged update after `from_offset`.
  ///
  /// Returns `Some(change)` when a new tagged update exists, otherwise `None`.
  fn changes<'a>(
    &'a self,
    tag: &'a str,
    from_offset: usize,
  ) -> DurableStateStoreFuture<'a, Option<DurableStateChange<A>>>;
}
