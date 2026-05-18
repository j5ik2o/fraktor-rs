use alloc::{boxed::Box, collections::VecDeque};

type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

/// A queued task paired with its affinity key for the inner executor.
pub(super) struct QueuedTask {
  pub(super) task:         BoxedTask,
  pub(super) affinity_key: u64,
}

/// Internal task queue state used by [`super::ExecutorShared`] to avoid re-entrant deadlocks.
pub struct TrampolineState {
  pub(super) pending: VecDeque<QueuedTask>,
}

impl TrampolineState {
  /// Creates an empty trampoline state.
  #[must_use]
  pub const fn new() -> Self {
    Self { pending: VecDeque::new() }
  }
}

impl Default for TrampolineState {
  fn default() -> Self {
    Self::new()
  }
}
