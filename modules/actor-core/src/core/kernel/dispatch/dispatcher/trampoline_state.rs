use alloc::{boxed::Box, collections::VecDeque};

type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

/// Internal task queue state used by [`super::ExecutorShared`] to avoid re-entrant deadlocks.
pub struct TrampolineState {
  pub(super) pending: VecDeque<BoxedTask>,
}

impl TrampolineState {
  /// Creates an empty trampoline state.
  #[must_use]
  pub fn new() -> Self {
    Self { pending: VecDeque::new() }
  }
}

impl Default for TrampolineState {
  fn default() -> Self {
    Self::new()
  }
}
