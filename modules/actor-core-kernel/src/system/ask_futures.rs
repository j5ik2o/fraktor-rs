//! Ask future registry for tracking pending ask operations.

#[cfg(test)]
#[path = "ask_futures_test.rs"]
mod tests;

use alloc::{collections::VecDeque, vec::Vec};

use fraktor_utils_core_rs::sync::SharedAccess;

use crate::{actor::messaging::AskResult, support::futures::ActorFutureShared};

/// Registry of pending ask futures.
pub(crate) struct AskFutures {
  futures: VecDeque<ActorFutureShared<AskResult>>,
}

const MAX_TRACKED_ASK_FUTURES: usize = 4096;

impl AskFutures {
  /// Creates a new empty ask futures registry.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { futures: VecDeque::new() }
  }

  /// Registers an ask future for tracking.
  pub(crate) fn push(&mut self, future: ActorFutureShared<AskResult>) {
    self.prune_ready();
    if self.futures.len() >= MAX_TRACKED_ASK_FUTURES {
      self.futures.pop_front();
    }
    self.futures.push_back(future);
  }

  /// Drains futures that have completed since the previous inspection.
  pub(crate) fn drain_ready(&mut self) -> Vec<ActorFutureShared<AskResult>> {
    let mut ready = Vec::new();
    self.prune_into(&mut ready);

    ready
  }

  fn prune_ready(&mut self) {
    self.futures.retain(|future| !future.with_read(|future| future.is_ready()));
  }

  fn prune_into(&mut self, drained: &mut Vec<ActorFutureShared<AskResult>>) {
    self.futures.retain(|future| {
      if future.with_read(|future| future.is_ready()) {
        drained.push(future.clone());
        false
      } else {
        true
      }
    });
  }
}

impl Default for AskFutures {
  fn default() -> Self {
    Self::new()
  }
}
