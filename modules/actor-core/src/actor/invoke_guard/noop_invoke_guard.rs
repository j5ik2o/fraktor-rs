//! Default no-op implementation of [`InvokeGuard`].

use super::InvokeGuard;
use crate::actor::error::ActorError;

/// No-op invoke guard used by default in `no_std`-compatible paths.
pub struct NoopInvokeGuard;

impl NoopInvokeGuard {
  /// Creates a no-op guard.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for NoopInvokeGuard {
  fn default() -> Self {
    Self::new()
  }
}

impl InvokeGuard for NoopInvokeGuard {
  fn wrap_receive(&self, call: &mut dyn FnMut() -> Result<(), ActorError>) -> Result<(), ActorError> {
    call()
  }
}
