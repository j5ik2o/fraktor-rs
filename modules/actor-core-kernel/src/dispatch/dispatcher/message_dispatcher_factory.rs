//! Trait that produces ready-to-use [`MessageDispatcherShared`] instances.

use super::message_dispatcher_shared::MessageDispatcherShared;

/// Trait describing a dispatcher factory.
///
/// Implementations are stored in the [`Dispatchers`](super::dispatchers::Dispatchers)
/// registry behind an `ArcShared` and called from spawn / bootstrap paths.
/// `dispatcher` must be `&self` so that the registry can hand the configurator
/// out without resorting to interior mutability.
pub trait MessageDispatcherFactory: Send + Sync {
  /// Returns a [`MessageDispatcherShared`] handle for the configured dispatcher.
  ///
  /// Concrete implementations decide whether to share a single dispatcher
  /// instance across calls (default / balancing) or build a new one each
  /// time (pinned).
  fn dispatcher(&self) -> MessageDispatcherShared;
}
