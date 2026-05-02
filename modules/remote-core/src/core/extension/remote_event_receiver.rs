//! Receiver port for [`crate::core::extension::RemoteEvent`] values.

use core::future::Future;

use crate::core::extension::RemoteEvent;

/// Pull-based event receiver consumed by [`crate::core::extension::Remote::run`].
pub trait RemoteEventReceiver: Send {
  /// Receives the next event, or `None` when the adapter-side sender has closed.
  fn recv(&mut self) -> impl Future<Output = Option<RemoteEvent>> + Send + '_;
}
