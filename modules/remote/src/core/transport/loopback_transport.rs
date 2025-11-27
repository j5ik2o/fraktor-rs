//! Loopback transport used for tests and no_std harnesses.

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::core::sync::ArcShared;

use super::{
  backpressure_hook::TransportBackpressureHookShared, remote_transport::RemoteTransport, transport_bind::TransportBind,
  transport_channel::TransportChannel, transport_endpoint::TransportEndpoint, transport_error::TransportError,
  transport_handle::TransportHandle, transport_inbound_handler::TransportInbound,
};

/// In-memory transport that records frames for assertions.
pub struct LoopbackTransport {
  state:   LoopbackState,
  hook:    Option<TransportBackpressureHookShared>,
  inbound: Option<ArcShared<dyn TransportInbound>>,
}

const PRESSURE_THRESHOLD: usize = 64;

struct LoopbackState {
  listeners:    BTreeMap<String, ListenerState>,
  channels:     BTreeMap<u64, String>,
  next_channel: u64,
}

struct ListenerState {
  frames: Vec<Vec<u8>>,
}

impl Default for LoopbackTransport {
  fn default() -> Self {
    Self {
      state:   LoopbackState { listeners: BTreeMap::new(), channels: BTreeMap::new(), next_channel: 1 },
      hook:    None,
      inbound: None,
    }
  }
}

impl LoopbackTransport {
  fn encode_frame(payload: &[u8], correlation_id: CorrelationId) -> Vec<u8> {
    let header_len = 12_usize;
    let total = header_len + payload.len();
    let mut frame = Vec::with_capacity(4 + total);
    frame.extend_from_slice(&(total as u32).to_be_bytes());
    frame.extend_from_slice(&correlation_id.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
  }

  fn fire_backpressure(&mut self, authority: &str, signal: BackpressureSignal, correlation_id: CorrelationId) {
    if let Some(ref hook_handle) = self.hook {
      let mut guard = hook_handle.lock();
      guard.on_backpressure(signal, authority, correlation_id);
    }
  }

  /// Test helper that drains frames recorded for the provided handle.
  #[cfg(any(test, feature = "test-support"))]
  pub fn drain_frames_for_test(&mut self, handle: &TransportHandle) -> Vec<Vec<u8>> {
    self
      .state
      .listeners
      .get_mut(handle.authority())
      .map(|listener| core::mem::take(&mut listener.frames))
      .unwrap_or_default()
  }

  /// Test helper to emit a backpressure signal without queue state thresholds.
  #[cfg(any(test, feature = "test-support"))]
  pub fn emit_backpressure_for_test(
    &mut self,
    authority: &str,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
  ) {
    self.fire_backpressure(authority, signal, correlation_id);
  }
}

impl RemoteTransport for LoopbackTransport {
  fn scheme(&self) -> &str {
    "fraktor.loopback"
  }

  fn spawn_listener(&mut self, bind: &TransportBind) -> Result<TransportHandle, TransportError> {
    self.state.listeners.entry(bind.authority().to_string()).or_insert_with(|| ListenerState { frames: Vec::new() });
    Ok(TransportHandle::new(bind.authority()))
  }

  fn open_channel(&mut self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError> {
    if !self.state.listeners.contains_key(endpoint.authority()) {
      return Err(TransportError::AuthorityNotBound(endpoint.authority().to_string()));
    }
    let id = self.state.next_channel;
    self.state.next_channel += 1;
    self.state.channels.insert(id, endpoint.authority().to_string());
    Ok(TransportChannel::new(id))
  }

  fn send(
    &mut self,
    channel: &TransportChannel,
    payload: &[u8],
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError> {
    let authority =
      self.state.channels.get(&channel.id()).ok_or(TransportError::ChannelUnavailable(channel.id()))?.clone();
    let listener =
      self.state.listeners.get_mut(&authority).ok_or_else(|| TransportError::AuthorityNotBound(authority.clone()))?;
    listener.frames.push(Self::encode_frame(payload, correlation_id));
    if listener.frames.len() >= PRESSURE_THRESHOLD {
      self.fire_backpressure(&authority, BackpressureSignal::Apply, correlation_id);
    }
    Ok(())
  }

  fn close(&mut self, channel: &TransportChannel) {
    self.state.channels.remove(&channel.id());
  }

  fn install_backpressure_hook(&mut self, hook: TransportBackpressureHookShared) {
    self.hook = Some(hook);
  }

  fn install_inbound_handler(&mut self, handler: ArcShared<dyn TransportInbound>) {
    self.inbound = Some(handler);
  }
}
