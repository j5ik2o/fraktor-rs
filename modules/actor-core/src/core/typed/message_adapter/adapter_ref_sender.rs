//! ActorRefSender implementation wrapping adapter payloads.

#[cfg(test)]
mod tests;

use alloc::format;
use core::any::Any;

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

    let (erased, sender, is_control, not_influence_receive_timeout) = message.into_parts();
    let payload = AdapterPayload::from_erased(erased);
    let envelope = AdapterEnvelope::new(payload, sender);
    // control フラグと NotInfluenceReceiveTimeout フラグの双方をアダプタ境界を越えて保持する。
    // `AnyMessage::not_influence` は marker trait 実装を要求するため `AdapterEnvelope` では使えないが、
    // `AnyMessage::from_parts` は raw bool を受け取るため marker trait なしでフラグを伝播できる。
    // sender は `AdapterEnvelope` 内に移動済みなので外側の `AnyMessage` は None で再構築する。
    let envelope_payload: ArcShared<dyn Any + Send + Sync + 'static> = ArcShared::new(envelope);
    let adapted = AnyMessage::from_parts(envelope_payload, None, is_control, not_influence_receive_timeout);

    match self.target.with_write(|target| target.send(adapted)) {
      | Ok(outcome) => Ok(outcome),
      | Err(error) => {
        self.system.record_send_error(Some(self.pid), &error);
        Err(error)
      },
    }
  }
}
