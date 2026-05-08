//! ActorRefSender implementation for runtime-owned message adapters.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, format};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};

use super::{AdapterLifecycleState, AdapterRefHandleId};
use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::{ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  event::logging::LogLevel,
  system::state::SystemStateShared,
};

type AdapterMessageWrapper = dyn Fn(AnyMessage) -> AnyMessage + Send + Sync + 'static;

pub(crate) struct AdapterRefSender {
  pid:       Pid,
  handle_id: AdapterRefHandleId,
  target:    ActorRefSenderShared,
  lifecycle: ArcShared<AdapterLifecycleState>,
  system:    SystemStateShared,
  wrap:      Box<AdapterMessageWrapper>,
}

impl AdapterRefSender {
  pub(crate) fn new(
    pid: Pid,
    handle_id: AdapterRefHandleId,
    target: ActorRefSenderShared,
    lifecycle: ArcShared<AdapterLifecycleState>,
    system: SystemStateShared,
    wrap: Box<AdapterMessageWrapper>,
  ) -> Self {
    Self { pid, handle_id, target, lifecycle, system, wrap }
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

    let adapted = (self.wrap)(message);
    match self.target.with_write(|target| target.send(adapted)) {
      | Ok(outcome) => Ok(outcome),
      | Err(error) => {
        self.system.record_send_error(Some(self.pid), &error);
        Err(error)
      },
    }
  }
}
