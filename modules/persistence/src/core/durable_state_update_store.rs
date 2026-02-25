//! Durable state update store abstraction.

use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

use crate::core::{durable_state_exception::DurableStateException, durable_state_store::DurableStateStore};

type DurableStateUpdateFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DurableStateException>> + Send + 'a>>;

/// Durable state store extension that exposes update notifications.
pub trait DurableStateUpdateStore<A>: DurableStateStore<A> {
  /// Loads the next update after `from_offset` for the persistence identifier.
  ///
  /// Returns `Some((next_offset, value))` when a new update exists, otherwise `None`.
  fn changes<'a>(
    &'a self,
    persistence_id: &'a str,
    from_offset: usize,
  ) -> DurableStateUpdateFuture<'a, Option<(usize, A)>>;
}
