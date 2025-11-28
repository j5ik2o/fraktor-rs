//! Hook invoked by transports when they need to signal throttling or release.

use alloc::boxed::Box;

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdMutex, sync::ArcShared};

/// Receives transport-level backpressure notifications.
pub trait TransportBackpressureHook: Send + Sync + 'static {
  /// Called whenever the transport requests throttling or resumes a remote authority.
  fn on_backpressure(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId);
}

/// Shared reference used to mutate a transport backpressure hook under a mutex.
pub type TransportBackpressureHookShared = ArcShared<NoStdMutex<Box<dyn TransportBackpressureHook>>>;
