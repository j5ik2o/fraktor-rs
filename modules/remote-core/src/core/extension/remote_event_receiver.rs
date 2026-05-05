//! Receiver port for [`crate::core::extension::RemoteEvent`] values.

use core::task::{Context, Poll};

use crate::core::extension::RemoteEvent;

/// Pull-based event receiver consumed by [`crate::core::extension::Remote::run`].
pub trait RemoteEventReceiver {
  /// Polls the next event, or `None` when the adapter-side sender has closed.
  fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<RemoteEvent>>;
}
