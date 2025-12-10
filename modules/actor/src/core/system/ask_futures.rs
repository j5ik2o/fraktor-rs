//! Ask future registry for tracking pending ask operations.

use alloc::vec::Vec;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::SharedAccess,
};

use crate::core::{futures::ActorFutureSharedGeneric, messaging::AnyMessageGeneric};

/// Registry of pending ask futures.
pub(crate) struct AskFuturesGeneric<TB: RuntimeToolbox + 'static> {
  futures: Vec<ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type AskFutures = AskFuturesGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> AskFuturesGeneric<TB> {
  /// Creates a new empty ask futures registry.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { futures: Vec::new() }
  }

  /// Registers an ask future for tracking.
  pub(crate) fn push(&mut self, future: ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>) {
    self.futures.push(future);
  }

  /// Drains futures that have completed since the previous inspection.
  pub(crate) fn drain_ready(&mut self) -> Vec<ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>> {
    let mut ready = Vec::new();
    let mut index = 0_usize;

    while index < self.futures.len() {
      if self.futures[index].with_read(|af| af.is_ready()) {
        ready.push(self.futures.swap_remove(index));
      } else {
        index += 1;
      }
    }

    ready
  }
}

impl<TB: RuntimeToolbox + 'static> Default for AskFuturesGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
