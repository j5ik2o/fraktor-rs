//! Shared dispatcher sender handle used by scheduler APIs.

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::dispatch::dispatcher::DispatcherSender;

/// Shared dispatcher sender reference resolved from actor contexts or system defaults.
#[derive(Clone)]
pub struct DispatcherSenderShared {
  inner: ArcShared<DispatcherSender>,
}

impl DispatcherSenderShared {
  /// Wraps a dispatcher sender inside the shared handle.
  #[must_use]
  pub const fn new(inner: ArcShared<DispatcherSender>) -> Self {
    Self { inner }
  }

  /// Returns a clone of the underlying dispatcher sender.
  #[must_use]
  pub fn sender(&self) -> ArcShared<DispatcherSender> {
    self.inner.clone()
  }
}
