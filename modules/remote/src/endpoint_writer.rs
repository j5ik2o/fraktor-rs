//! Converts outbound envelopes into serialized remoting frames.

use fraktor_actor_rs::core::{
  actor_prim::actor_path::ActorPathParts,
  event_stream::BackpressureSignal,
  messaging::{AnyMessageGeneric, SystemMessage},
  serialization::{SerializationCallScope, SerializationError, SerializationExtensionGeneric, SerializedMessage},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::RemoteNodeId;
use self::outbound_queue::{EnvelopePriority, OutboundQueue};

pub mod outbound_queue;

/// Envelope emitted by the endpoint writer, ready for transport serialization.
pub struct RemotingEnvelope {
  target: ActorPathParts,
  remote: RemoteNodeId,
  payload: SerializedMessage,
  reply_to: Option<ActorPathParts>,
}

impl RemotingEnvelope {
  /// Returns target actor path parts.
  #[must_use]
  pub fn target(&self) -> &ActorPathParts {
    &self.target
  }

  /// Returns the remote node metadata.
  #[must_use]
  pub fn remote(&self) -> &RemoteNodeId {
    &self.remote
  }

  /// Returns serialized payload bytes.
  #[must_use]
  pub fn payload(&self) -> &SerializedMessage {
    &self.payload
  }

  /// Returns reply-to actor path when available.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorPathParts> {
    self.reply_to.as_ref()
  }
}

/// Outbound envelope submitted to the writer.
pub struct OutboundEnvelope<TB: RuntimeToolbox + 'static> {
  /// Destination actor path.
  pub target: ActorPathParts,
  /// Remote node metadata.
  pub remote: RemoteNodeId,
  /// Message payload.
  pub message: AnyMessageGeneric<TB>,
}

/// Serializes outbound envelopes using the actor serialization extension.
pub struct EndpointWriter<TB: RuntimeToolbox + 'static> {
  serialization: ArcShared<SerializationExtensionGeneric<TB>>,
  queue:         OutboundQueue<TB, OutboundEnvelope<TB>>,
}

impl<TB: RuntimeToolbox + 'static> EndpointWriter<TB> {
  /// Creates a writer backed by the provided serialization extension.
  #[must_use]
  pub fn new(serialization: ArcShared<SerializationExtensionGeneric<TB>>) -> Self {
    Self { serialization, queue: OutboundQueue::new() }
  }

  /// Serializes the outbound envelope into a remoting envelope.
  pub fn write(&self, envelope: OutboundEnvelope<TB>) -> Result<RemotingEnvelope, SerializationError> {
    let payload = self
      .serialization
      .serialize(envelope.message.payload(), SerializationCallScope::Remote)?;
    let reply_to = envelope
      .message
      .reply_to()
      .and_then(|reply| reply.path().map(|path| path.parts().clone()));

    Ok(RemotingEnvelope { target: envelope.target, remote: envelope.remote, payload, reply_to })
  }

  /// Enqueues an envelope for later transmission.
  pub fn enqueue(&mut self, envelope: OutboundEnvelope<TB>) {
    self.queue.push(envelope, |env| {
      if env.message.payload().is::<SystemMessage>() {
        EnvelopePriority::System
      } else {
        EnvelopePriority::User
      }
    });
  }

  /// Pops the next envelope respecting system priority.
  #[must_use]
  pub fn dequeue(&mut self) -> Option<OutboundEnvelope<TB>> {
    self.queue.pop()
  }

  /// Applies transport backpressure signals to pause/resume user traffic.
  pub fn notify_backpressure(&mut self, signal: BackpressureSignal) {
    match signal {
      | BackpressureSignal::Apply => self.queue.pause_user(),
      | BackpressureSignal::Release => self.queue.resume_user(),
    }
  }
}

#[cfg(test)]
mod tests;
