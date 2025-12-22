//! Activation storage trait for std implementations.

use std::{boxed::Box, future::Future, pin::Pin};

use crate::core::{ActivationEntry, ActivationStorageError, GrainKey};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Persistent activation storage abstraction.
pub trait ActivationStorage {
  /// Loads activation entry for the given key.
  fn load<'a>(
    &'a mut self,
    key: &'a GrainKey,
  ) -> BoxFuture<'a, Result<Option<ActivationEntry>, ActivationStorageError>>;

  /// Stores activation entry for the given key.
  fn store<'a>(
    &'a mut self,
    key: &'a GrainKey,
    entry: ActivationEntry,
  ) -> BoxFuture<'a, Result<(), ActivationStorageError>>;

  /// Removes activation entry for the given key.
  fn remove<'a>(&'a mut self, key: &'a GrainKey) -> BoxFuture<'a, Result<(), ActivationStorageError>>;
}
