//! Activation executor trait used by placement coordination.

use alloc::string::String;
use core::future::Future;

use crate::core::{ActivationError, ActivationRecord, GrainKey};

/// Executes or ensures activation creation.
pub trait ActivationExecutor {
  /// Future returned by [`ActivationExecutor::ensure_activation`].
  type EnsureActivationFuture<'a>: Future<Output = Result<ActivationRecord, ActivationError>> + Send + 'a
  where
    Self: 'a;

  /// Ensures activation for the given key.
  fn ensure_activation<'a>(&'a mut self, key: GrainKey, owner: String) -> Self::EnsureActivationFuture<'a>;
}
