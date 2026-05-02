//! Tokio MPSC-backed remote event receiver.

use core::future::Future;

use fraktor_remote_core_rs::core::extension::{RemoteEvent, RemoteEventReceiver};
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
  fn recv(&mut self) -> impl Future<Output = Option<RemoteEvent>> + Send + '_ {
    self.receiver.recv()
  }
}
