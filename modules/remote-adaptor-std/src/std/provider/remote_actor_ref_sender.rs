//! Adapter sender that enqueues remote-bound messages into the core event loop.

use std::time::Instant;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  event::stream::CorrelationId,
};
use fraktor_remote_core_rs::core::{
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::RemoteEvent,
  provider::RemoteActorRef,
  transport::TransportEndpoint,
};
use tokio::sync::mpsc::{Sender, error::TrySendError};

use crate::std::association::std_instant_elapsed_millis;

/// Sender that wraps a [`RemoteActorRef`] and pushes outbound work to `Remote`.
pub struct RemoteActorRefSender {
  remote_ref:      RemoteActorRef,
  event_tx:        Sender<RemoteEvent>,
  monotonic_epoch: Instant,
}

impl RemoteActorRefSender {
  /// Creates a new sender for the given `remote_ref`.
  #[must_use]
  pub fn new(remote_ref: RemoteActorRef, event_tx: Sender<RemoteEvent>, monotonic_epoch: Instant) -> Self {
    Self { remote_ref, event_tx, monotonic_epoch }
  }

  fn remote_authority(&self) -> Option<String> {
    let remote_node = self.remote_ref.remote_node();
    let port = remote_node.port()?;
    Some(alloc::format!("{}@{}:{}", remote_node.system(), remote_node.host(), port))
  }

  fn outbound_envelope(&self, message: AnyMessage) -> Result<(TransportEndpoint, OutboundEnvelope), SendError> {
    let Some(authority) = self.remote_authority() else {
      return Err(SendError::invalid_payload(message, "remote actor ref is missing a transport port"));
    };
    let sender = message.sender().and_then(|actor_ref| actor_ref.canonical_path());
    let envelope = OutboundEnvelope::new(
      self.remote_ref.path().clone(),
      sender,
      message,
      OutboundPriority::User,
      self.remote_ref.remote_node().clone(),
      CorrelationId::nil(),
    );
    Ok((TransportEndpoint::new(authority), envelope))
  }

  fn recover_message(event: RemoteEvent) -> Option<AnyMessage> {
    match event {
      | RemoteEvent::OutboundEnqueued { envelope, .. } => {
        let (_recipient, _sender, message, _priority, _remote_node, _correlation_id) = envelope.into_parts();
        Some(message)
      },
      | _ => None,
    }
  }
}

impl ActorRefSender for RemoteActorRefSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let (authority, envelope) = self.outbound_envelope(message)?;
    let event = RemoteEvent::OutboundEnqueued {
      authority,
      envelope: Box::new(envelope),
      now_ms: std_instant_elapsed_millis(self.monotonic_epoch),
    };
    match self.event_tx.try_send(event) {
      | Ok(()) => Ok(SendOutcome::Delivered),
      | Err(TrySendError::Full(event)) => match Self::recover_message(event) {
        | Some(message) => Err(SendError::full(message)),
        | None => unreachable!("remote sender only sends OutboundEnqueued events"),
      },
      | Err(TrySendError::Closed(event)) => match Self::recover_message(event) {
        | Some(message) => Err(SendError::closed(message)),
        | None => unreachable!("remote sender only sends OutboundEnqueued events"),
      },
    }
  }
}
