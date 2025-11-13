use alloc::boxed::Box;

use crate::{sync::ArcShared, timing::delay::DelayState};

/// Handle owned by providers to complete a delay.
#[derive(Clone)]
pub struct DelayTrigger {
  state: ArcShared<DelayState>,
}

impl DelayTrigger {
  /// Creates a new trigger with the given state.
  pub(crate) const fn new(state: ArcShared<DelayState>) -> Self {
    Self { state }
  }

  /// Completes the associated delay future.
  pub fn fire(&self) {
    self.state.complete();
  }

  /// Registers a cancellation hook that will run if the future is dropped before completion.
  pub fn set_cancel_hook<F>(&self, hook: F)
  where
    F: FnOnce() + Send + Sync + 'static, {
    self.state.install_cancel_hook(Box::new(hook));
  }
}
