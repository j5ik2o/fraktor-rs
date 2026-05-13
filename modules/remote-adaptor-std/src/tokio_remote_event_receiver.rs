//! Tokio MPSC-backed remote event receiver.

use core::task::{Context, Poll};

use fraktor_remote_core_rs::extension::{RemoteEvent, RemoteEventReceiver};
use tokio::sync::mpsc::Receiver;

/// [`RemoteEventReceiver`] implementation backed by `tokio::sync::mpsc`.
pub struct TokioMpscRemoteEventReceiver {
  receiver: Receiver<RemoteEvent>,
}

impl TokioMpscRemoteEventReceiver {
  /// Creates a new receiver wrapper.
  #[must_use]
  pub const fn new(receiver: Receiver<RemoteEvent>) -> Self {
    Self { receiver }
  }
}

impl RemoteEventReceiver for TokioMpscRemoteEventReceiver {
  fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<RemoteEvent>> {
    self.receiver.poll_recv(cx)
  }
}
