//! Converts outbound envelopes into serialized remoting frames.

use fraktor_actor_rs::core::{
  event_stream::BackpressureSignal,
  messaging::SystemMessage,
  serialization::{SerializationCallScope, SerializationError, SerializationExtensionGeneric},
};
use fraktor_utils_rs::core::{collections::queue::QueueError, runtime_toolbox::RuntimeToolbox, sync::ArcShared};

mod envelope_priority;
mod outbound_envelope;
mod outbound_queue;
mod remoting_envelope;

#[cfg(test)]
mod tests;

pub use envelope_priority::EnvelopePriority;
pub use outbound_envelope::OutboundEnvelope;
use outbound_queue::OutboundQueue;
pub use remoting_envelope::RemotingEnvelope;

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
    let payload = self.serialization.serialize(envelope.message.payload(), SerializationCallScope::Remote)?;
    let reply_to = envelope.message.reply_to().and_then(|reply| reply.path().map(|path| path.parts().clone()));

    Ok(RemotingEnvelope { target: envelope.target, remote: envelope.remote, payload, reply_to })
  }

  /// Enqueues an envelope for later transmission.
  #[allow(clippy::result_large_err)]
  pub fn enqueue(&mut self, envelope: OutboundEnvelope<TB>) -> Result<(), QueueError<OutboundEnvelope<TB>>> {
    self.queue.push(envelope, |env| {
      if env.message.payload().is::<SystemMessage>() { EnvelopePriority::System } else { EnvelopePriority::User }
    })
  }

  /// Pops the next envelope respecting system priority.
  #[allow(clippy::result_large_err)]
  pub fn dequeue(&mut self) -> Result<Option<OutboundEnvelope<TB>>, QueueError<OutboundEnvelope<TB>>> {
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
