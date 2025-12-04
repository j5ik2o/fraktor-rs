//! Observers notified when transports request backpressure adjustments.

use alloc::boxed::Box;

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::core::{runtime_toolbox::ToolboxMutex, sync::ArcShared};

/// Listener invoked whenever backpressure is applied or released for a remote authority.
///
/// # External Synchronization
///
/// Callers should wrap implementations in a mutex (provided by the runtime
/// toolbox) and call `on_signal` through a mutable guard.
pub trait RemotingBackpressureListener: Send + Sync + 'static {
  /// Called with the latest backpressure signal, authority identifier, and correlation id.
  fn on_signal(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId);

  /// Clones the listener as a boxed trait object.
  fn clone_box(&self) -> Box<dyn RemotingBackpressureListener>;
}

/// Shared handle to a [`RemotingBackpressureListener`] protected by the runtime mutex family.
pub(crate) type RemotingBackpressureListenerShared<TB> =
  ArcShared<ToolboxMutex<Box<dyn RemotingBackpressureListener>, TB>>;
