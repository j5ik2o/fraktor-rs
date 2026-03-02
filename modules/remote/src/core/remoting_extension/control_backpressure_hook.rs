use fraktor_actor_rs::core::event::stream::{BackpressureSignal, CorrelationId};

use super::control_handle::RemotingControlHandle;
use crate::core::transport::TransportBackpressureHook;

#[allow(dead_code)]
pub(super) struct ControlBackpressureHook {
  pub(super) control: RemotingControlHandle,
}

impl TransportBackpressureHook for ControlBackpressureHook {
  fn on_backpressure(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId) {
    self.control.notify_backpressure(authority, signal, Some(correlation_id));
  }
}
