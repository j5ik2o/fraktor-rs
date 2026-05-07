//! Connection-loss event emission for TCP I/O tasks.

use std::time::Instant;

use fraktor_remote_core_rs::core::{
  extension::RemoteEvent,
  transport::{TransportEndpoint, TransportError},
};
use tokio::sync::mpsc::Sender;

use crate::std::association::std_instant_elapsed_millis;

#[derive(Clone)]
pub(super) struct ConnectionLossReporter {
  sender:          Sender<RemoteEvent>,
  authority:       TransportEndpoint,
  monotonic_epoch: Instant,
}

impl ConnectionLossReporter {
  pub(super) const fn new(sender: Sender<RemoteEvent>, authority: TransportEndpoint, monotonic_epoch: Instant) -> Self {
    Self { sender, authority, monotonic_epoch }
  }

  pub(super) async fn report(&self, cause: TransportError) {
    let event = RemoteEvent::ConnectionLost {
      authority: self.authority.clone(),
      cause,
      now_ms: std_instant_elapsed_millis(self.monotonic_epoch),
    };
    if let Err(error) = self.sender.send(event).await {
      tracing::warn!(?error, authority = %self.authority.authority(), "connection-lost event delivery failed");
    }
  }
}
