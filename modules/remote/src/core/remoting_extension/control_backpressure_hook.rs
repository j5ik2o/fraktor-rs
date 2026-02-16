use fraktor_actor_rs::core::event::stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::control_handle::RemotingControlHandle;
use crate::core::transport::TransportBackpressureHook;

#[allow(dead_code)]
pub(super) struct ControlBackpressureHook<TB>
where
  TB: RuntimeToolbox + 'static, {
  pub(super) control: RemotingControlHandle<TB>,
}

impl<TB> TransportBackpressureHook for ControlBackpressureHook<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn on_backpressure(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId) {
    self.control.notify_backpressure(authority, signal, Some(correlation_id));
  }
}
