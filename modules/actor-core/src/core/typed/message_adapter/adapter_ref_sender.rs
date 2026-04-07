//! ActorRefSender implementation wrapping adapter payloads.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};

use crate::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRefSender, ActorRefSenderShared, SendOutcome},
      error::SendError,
      messaging::AnyMessage,
    },
    event::logging::LogLevel,
    system::state::SystemStateShared,
  },
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
      self.system.emit_log(LogLevel::Warn, log, Some(self.pid), None);
      return Err(error);
    }

    let (erased, sender, is_control) = message.into_parts();
    let payload = AdapterPayload::from_erased(erased);
    let envelope = AdapterEnvelope::new(payload, sender);
    // アダプタ境界を越えても control フラグを保持する
    let adapted = if is_control { AnyMessage::control(envelope) } else { AnyMessage::new(envelope) };

    match self.target.with_write(|target| target.send(adapted)) {
      | Ok(outcome) => Ok(outcome),
      | Err(error) => {
        self.system.record_send_error(Some(self.pid), &error);
        Err(error)
      },
    }
  }
}
