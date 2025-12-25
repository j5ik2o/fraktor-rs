//! ActorRefSender implementation wrapping adapter payloads.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::{
  actor::{
    Pid,
    actor_ref::{ActorRefSender, ActorRefSenderSharedGeneric, SendOutcome},
  },
  error::SendError,
  event::logging::LogLevel,
  messaging::AnyMessageGeneric,
  system::SystemStateSharedGeneric,
  typed::message_adapter::{AdapterEnvelope, AdapterLifecycleState, AdapterPayload, AdapterRefHandleId},
};

/// Sends external messages through the adapter pipeline.
pub struct AdapterRefSender<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  pid:       Pid,
  handle_id: AdapterRefHandleId,
  target:    ActorRefSenderSharedGeneric<TB>,
  lifecycle: ArcShared<AdapterLifecycleState<TB>>,
  system:    SystemStateSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> AdapterRefSender<TB> {
  /// Creates a new sender instance.
  #[must_use]
  pub const fn new(
    pid: Pid,
    handle_id: AdapterRefHandleId,
    target: ActorRefSenderSharedGeneric<TB>,
    lifecycle: ArcShared<AdapterLifecycleState<TB>>,
    system: SystemStateSharedGeneric<TB>,
  ) -> Self {
    Self { pid, handle_id, target, lifecycle, system }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for AdapterRefSender<TB> {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    if !self.lifecycle.is_alive() {
      let error = SendError::closed(message);
      self.system.record_send_error(Some(self.pid), &error);
      let log = format!("adapter-ref-{} target stopped", self.handle_id.get());
      self.system.emit_log(LogLevel::Warn, log, Some(self.pid));
      return Err(error);
    }

    let (erased, sender) = message.into_payload_and_sender();
    let payload = AdapterPayload::from_erased(erased);
    let envelope = AdapterEnvelope::new(payload, sender);
    let adapted = AnyMessageGeneric::new(envelope);

    match self.target.send(adapted) {
      | Ok(()) => Ok(SendOutcome::Delivered),
      | Err(error) => {
        self.system.record_send_error(Some(self.pid), &error);
        Err(error)
      },
    }
  }
}
