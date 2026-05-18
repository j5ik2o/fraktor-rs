//! Factory abstraction that produces fresh [`ExecutorShared`] instances.

use super::executor_shared::ExecutorShared;

/// Builds an [`ExecutorShared`] for a dispatcher.
///
/// Implementations are usually parameterised by id (e.g., `pekko.actor.default-dispatcher`).
/// The factory always returns an `ExecutorShared` rather than the raw executor so
/// that ownership flows through the AShared wrapper.
pub trait ExecutorFactory: Send + Sync {
  /// Creates an [`ExecutorShared`] dedicated to the dispatcher named `id`.
  fn create(&self, id: &str) -> ExecutorShared;
}
