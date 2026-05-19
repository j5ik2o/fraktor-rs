//! Receiver port for [`crate::extension::RemoteEvent`] values.

use core::task::{Context, Poll};

use crate::extension::RemoteEvent;

/// Pull-based event receiver consumed by [`crate::extension::Remote::run`].
pub trait RemoteEventReceiver {
  /// Polls the next event, or `None` when the adapter-side sender has closed.
  fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<RemoteEvent>>;
}
