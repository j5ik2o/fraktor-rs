//! Placeholder loopback transport implementation used for tests.

use alloc::{collections::BTreeMap, string::{String, ToString}, vec::Vec};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use super::{BackpressureHook, RemoteTransport, TransportBind, TransportChannel, TransportEndpoint, TransportError, TransportHandle};

struct LoopbackInner {
  listeners: ToolboxMutex<BTreeMap<String, TransportHandle>, NoStdToolbox>,
  hook:      ToolboxMutex<Option<BackpressureHook>, NoStdToolbox>,
  threshold: usize,
}

impl LoopbackInner {
  fn new() -> Self {
    Self {
      listeners: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(BTreeMap::new()),
      hook: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(None),
      threshold: 8,
    }
  }

  fn notify(&self, signal: fraktor_actor_rs::core::event_stream::BackpressureSignal, authority: &str) {
    if let Some(hook) = self.hook.lock().as_ref() {
      hook(signal, authority);
    }
  }
}

/// In-process transport used for early integration testing.
#[derive(Clone)]
pub struct LoopbackTransport {
  inner: ArcShared<LoopbackInner>,
}

impl LoopbackTransport {
  /// Creates a new loopback transport instance.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(LoopbackInner::new()) }
  }

  fn listener(&self, authority: &str) -> Option<TransportHandle> {
    self.inner.listeners.lock().get(authority).cloned()
  }

  fn enforce_backpressure(&self, authority: &str, buffered: usize) {
    use fraktor_actor_rs::core::event_stream::BackpressureSignal;
    if buffered > self.inner.threshold {
      self.inner.notify(BackpressureSignal::Apply, authority);
    } else if buffered <= 1 {
      self.inner.notify(BackpressureSignal::Release, authority);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> RemoteTransport<TB> for LoopbackTransport {
  fn scheme(&self) -> &str {
    "fraktor.loopback"
  }

  fn install_backpressure_hook(&self, hook: BackpressureHook) {
    *self.inner.hook.lock() = Some(hook);
  }

  fn spawn_listener(&self, bind: &TransportBind) -> Result<TransportHandle, TransportError> {
    let handle = TransportHandle::new(bind.authority());
    self.inner.listeners.lock().insert(bind.authority().to_string(), handle.clone());
    Ok(handle)
  }

  fn open_channel(&self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError> {
    if self.listener(endpoint.authority()).is_some() {
      Ok(TransportChannel::new(endpoint.authority()))
    } else {
      Err(TransportError::ChannelUnavailable)
    }
  }

  fn send(&self, channel: &TransportChannel, payload: &[u8]) -> Result<(), TransportError> {
    let listener = self.listener(channel.authority()).ok_or(TransportError::SendFailed)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    let len = payload.len() as u32;
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(payload);
    listener.push_frame(frame);
    self.enforce_backpressure(channel.authority(), listener.buffered());
    Ok(())
  }

  fn close(&self, _channel: TransportChannel) {
    // nothing to do for loopback
  }
}
