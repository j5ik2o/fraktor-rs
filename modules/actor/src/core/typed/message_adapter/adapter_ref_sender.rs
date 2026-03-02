//! ActorRefSender implementation wrapping adapter payloads.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  actor::{
    Pid,
    actor_ref::{ActorRefSender, ActorRefSenderShared, SendOutcome},
  },
  error::SendError,
  event::logging::LogLevel,
  messaging::AnyMessage,
  system::state::SystemStateShared,
  typed::message_adapter::{AdapterEnvelope, AdapterLifecycleState, AdapterPayload, AdapterRefHandleId},
};

/// Sends external messages through the adapter pipeline.
pub(crate) struct AdapterRefSender {
  pid:       Pid,
  handle_id: AdapterRefHandleId,
  target:    ActorRefSenderShared,
  lifecycle: ArcShared<AdapterLifecycleState>,
  system:    SystemStateShared,
}

impl AdapterRefSender {
  /// Creates a new sender instance.
  #[must_use]
  pub(crate) const fn new(
    pid: Pid,
    handle_id: AdapterRefHandleId,
    target: ActorRefSenderShared,
    lifecycle: ArcShared<AdapterLifecycleState>,
    system: SystemStateShared,
  ) -> Self {
    Self { pid, handle_id, target, lifecycle, system }
  }
}

impl ActorRefSender for AdapterRefSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if !self.lifecycle.is_alive() {
      let error = SendError::closed(message);
      self.system.record_send_error(Some(self.pid), &error);
      let log = format!("adapter-ref-{} target stopped", self.handle_id);
      self.system.emit_log(LogLevel::Warn, log, Some(self.pid));
      return Err(error);
    }

    let (erased, sender) = message.into_payload_and_sender();
    let payload = AdapterPayload::from_erased(erased);
    let envelope = AdapterEnvelope::new(payload, sender);
    let adapted = AnyMessage::new(envelope);

    match self.target.send(adapted) {
      | Ok(()) => Ok(SendOutcome::Delivered),
      | Err(error) => {
        self.system.record_send_error(Some(self.pid), &error);
        Err(error)
      },
    }
  }
}
