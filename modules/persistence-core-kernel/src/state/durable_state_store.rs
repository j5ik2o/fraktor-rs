//! Durable state store abstraction.

use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

use crate::state::{DurableStateError, GetObjectResult};

pub(crate) type DurableStateStoreFuture<'a, T> =
  Pin<Box<dyn Future<Output = Result<T, DurableStateError>> + Send + 'a>>;

/// Durable state store abstraction using object-safe boxed futures.
pub trait DurableStateStore<A: Send>: Send + Sync + 'static {
  /// Loads the durable state object for the persistence identifier.
  fn get_object<'a>(&'a self, persistence_id: &'a str) -> DurableStateStoreFuture<'a, GetObjectResult<A>>;

  /// Inserts or updates the durable state object for the persistence identifier.
  fn upsert_object<'a>(
    &'a mut self,
    persistence_id: &'a str,
    expected_revision: u64,
    object: A,
    tag: Option<&'a str>,
  ) -> DurableStateStoreFuture<'a, ()>;

  /// Deletes the durable state object for the persistence identifier.
  ///
  /// A successful delete removes the object and resets the stored revision to
  /// `0`, so the next upsert for the same persistence identifier uses expected
  /// revision `0`.
  fn delete_object<'a>(
    &'a mut self,
    persistence_id: &'a str,
    expected_revision: u64,
  ) -> DurableStateStoreFuture<'a, ()>;
}
