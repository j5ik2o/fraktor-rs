//! ActorRefSender implementation wrapping adapter payloads.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_utils_core_rs::sync::{ArcShared, NoStdToolbox};

use crate::{
  RuntimeToolbox,
  actor_prim::{Pid, actor_ref::ActorRefSender},
  error::SendError,
  logging::LogLevel,
  messaging::AnyMessageGeneric,
  system::SystemStateGeneric,
  typed::message_adapter::{AdapterEnvelope, AdapterLifecycleState, AdapterPayload, AdapterRefHandleId},
};

/// Sends external messages through the adapter pipeline.
pub struct AdapterRefSender<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  pid:       Pid,
  handle_id: AdapterRefHandleId,
  target:    ArcShared<dyn ActorRefSender<TB>>,
  lifecycle: ArcShared<AdapterLifecycleState<TB>>,
  system:    ArcShared<SystemStateGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AdapterRefSender<TB> {
  /// Creates a new sender instance.
  #[must_use]
  pub fn new(
    pid: Pid,
    handle_id: AdapterRefHandleId,
    target: ArcShared<dyn ActorRefSender<TB>>,
    lifecycle: ArcShared<AdapterLifecycleState<TB>>,
    system: ArcShared<SystemStateGeneric<TB>>,
  ) -> Self {
    Self { pid, handle_id, target, lifecycle, system }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for AdapterRefSender<TB> {
  fn send(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    if !self.lifecycle.is_alive() {
      let error = SendError::closed(message);
      self.system.record_send_error(Some(self.pid), &error);
      let log = format!("adapter-ref-{} target stopped", self.handle_id.get());
      self.system.emit_log(LogLevel::Warn, log, Some(self.pid));
      return Err(error);
    }

    let (erased, reply_to) = message.into_payload_and_reply();
    let payload = AdapterPayload::from_erased(erased);
    let envelope = AdapterEnvelope::new(payload, reply_to);
    let adapted = AnyMessageGeneric::new(envelope);

    match self.target.send(adapted) {
      | Ok(()) => Ok(()),
      | Err(error) => {
        self.system.record_send_error(Some(self.pid), &error);
        Err(error)
      },
    }
  }
}
