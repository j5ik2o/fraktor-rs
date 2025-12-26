//! Activation storage trait used by placement coordination.

use core::future::Future;

use crate::core::{ActivationEntry, ActivationStorageError, GrainKey};

/// Persistent activation storage abstraction.
pub trait ActivationStorage {
  /// Future returned by [`ActivationStorage::load`].
  type LoadFuture<'a>: Future<Output = Result<Option<ActivationEntry>, ActivationStorageError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by [`ActivationStorage::store`].
  type StoreFuture<'a>: Future<Output = Result<(), ActivationStorageError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by [`ActivationStorage::remove`].
  type RemoveFuture<'a>: Future<Output = Result<(), ActivationStorageError>> + Send + 'a
  where
    Self: 'a;

  /// Loads activation entry for the given key.
  fn load<'a>(&'a mut self, key: GrainKey) -> Self::LoadFuture<'a>;

  /// Stores activation entry for the given key.
  fn store<'a>(&'a mut self, key: GrainKey, entry: ActivationEntry) -> Self::StoreFuture<'a>;

  /// Removes activation entry for the given key.
  fn remove<'a>(&'a mut self, key: GrainKey) -> Self::RemoveFuture<'a>;
}
