//! Activation executor trait for std implementations.

use std::{boxed::Box, future::Future, pin::Pin};

use crate::core::{ActivationError, ActivationRecord, GrainKey};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Executes or ensures activation creation.
pub trait ActivationExecutor {
  /// Ensures activation for the given key.
  fn ensure_activation<'a>(
    &'a mut self,
    key: &'a GrainKey,
    owner: &'a str,
  ) -> BoxFuture<'a, Result<ActivationRecord, ActivationError>>;
}
