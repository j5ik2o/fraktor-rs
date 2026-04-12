//! Factory contract for [`ExecutorShared`](super::ExecutorShared).

use alloc::boxed::Box;

use super::{Executor, ExecutorShared, TrampolineState};

/// Materializes [`ExecutorShared`] instances.
pub trait ExecutorSharedFactory: Send + Sync {
  /// Creates a shared executor wrapper.
  fn create_executor_shared(&self, executor: Box<dyn Executor>, trampoline: TrampolineState) -> ExecutorShared;
}
