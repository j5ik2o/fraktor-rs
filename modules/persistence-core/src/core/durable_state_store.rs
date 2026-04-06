//! Durable state store abstraction.

use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

use crate::core::durable_state_exception::DurableStateException;

pub(crate) type DurableStateStoreFuture<'a, T> =
  Pin<Box<dyn Future<Output = Result<T, DurableStateException>> + Send + 'a>>;

/// Durable state store abstraction using object-safe boxed futures.
pub trait DurableStateStore<A: Send>: Send + Sync + 'static {
  /// Loads the durable state object for the persistence identifier.
  fn get_object<'a>(&'a self, persistence_id: &'a str) -> DurableStateStoreFuture<'a, Option<A>>;

  /// Inserts or updates the durable state object for the persistence identifier.
  fn upsert_object<'a>(&'a mut self, persistence_id: &'a str, object: A) -> DurableStateStoreFuture<'a, ()>;

  /// Deletes the durable state object for the persistence identifier.
  fn delete_object<'a>(&'a mut self, persistence_id: &'a str) -> DurableStateStoreFuture<'a, ()>;
}
