//! Shared dispatcher sender handle used by scheduler APIs.

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::{RuntimeToolbox, dispatcher::DispatcherSenderGeneric};

/// Shared dispatcher sender reference resolved from actor contexts or system defaults.
#[derive(Clone)]
pub struct DispatcherSenderShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<DispatcherSenderGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> DispatcherSenderShared<TB> {
  /// Wraps a dispatcher sender inside the shared handle.
  #[must_use]
  pub const fn new(inner: ArcShared<DispatcherSenderGeneric<TB>>) -> Self {
    Self { inner }
  }

  /// Returns a clone of the underlying dispatcher sender.
  #[must_use]
  pub fn sender(&self) -> ArcShared<DispatcherSenderGeneric<TB>> {
    self.inner.clone()
  }
}
