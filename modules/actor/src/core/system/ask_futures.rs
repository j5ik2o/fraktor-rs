//! Ask future registry for tracking pending ask operations.

use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::{futures::ActorFutureShared, messaging::AskResult};

/// Registry of pending ask futures.
pub(crate) struct AskFutures {
  futures: Vec<ActorFutureShared<AskResult>>,
}
#[allow(dead_code)]
impl AskFutures {
  /// Creates a new empty ask futures registry.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { futures: Vec::new() }
  }

  /// Registers an ask future for tracking.
  pub(crate) fn push(&mut self, future: ActorFutureShared<AskResult>) {
    self.futures.push(future);
  }

  /// Drains futures that have completed since the previous inspection.
  pub(crate) fn drain_ready(&mut self) -> Vec<ActorFutureShared<AskResult>> {
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

impl Default for AskFutures {
  fn default() -> Self {
    Self::new()
  }
}
