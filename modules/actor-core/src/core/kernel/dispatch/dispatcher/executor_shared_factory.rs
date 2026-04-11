//! Factory contract for [`ExecutorShared`](super::ExecutorShared).

use alloc::boxed::Box;

use super::{Executor, ExecutorShared};

/// Materializes [`ExecutorShared`] instances.
pub trait ExecutorSharedFactory: Send + Sync {
  /// Creates a shared executor wrapper.
  fn create(&self, executor: Box<dyn Executor>) -> ExecutorShared;
}
