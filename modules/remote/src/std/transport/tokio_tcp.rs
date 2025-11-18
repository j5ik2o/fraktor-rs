//! Tokio TCP transport for production remoting.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::fmt;

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdMutex, sync::ArcShared};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  runtime::Handle,
  sync::mpsc,
};

use crate::core::{
  RemoteTransport, TransportBackpressureHook, TransportBind, TransportChannel, TransportEndpoint, TransportError,
  TransportHandle,
};

const BACKPRESSURE_THRESHOLD: usize = 1024;
const CHANNEL_BUFFER_SIZE: usize = 256;

/// Tokio-based TCP transport implementing the Pekko wire protocol.
pub struct TokioTcpTransport {
  state:   ArcShared<NoStdMutex<TokioTcpState>>,
  hook:    ArcShared<NoStdMutex<Option<ArcShared<dyn TransportBackpressureHook>>>>,
  runtime: Handle,
}

struct TokioTcpState {
  listeners:    BTreeMap<String, ListenerHandle>,
  channels:     BTreeMap<u64, ChannelHandle>,
  next_channel: u64,
}

struct ListenerHandle {
  _task: tokio::task::JoinHandle<()>,
}

struct ChannelHandle {
  authority: String,
  sender:    mpsc::Sender<OutboundFrame>,
}

struct OutboundFrame {
  payload:        Vec<u8>,
  correlation_id: CorrelationId,
}

impl fmt::Debug for TokioTcpTransport {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TokioTcpTransport").finish_non_exhaustive()
  }
}

impl Default for TokioTcpTransport {
  fn default() -> Self {
    Self::new()
  }
}

impl TokioTcpTransport {
  /// Creates a new Tokio TCP transport using the current runtime.
  #[must_use]
  pub fn new() -> Self {
    Self::with_runtime(Handle::current())
  }

  /// Creates a new Tokio TCP transport with the specified runtime handle.
  #[must_use]
  pub fn with_runtime(runtime: Handle) -> Self {
    Self {
      state: ArcShared::new(NoStdMutex::new(TokioTcpState {
        listeners:    BTreeMap::new(),
        channels:     BTreeMap::new(),
        next_channel: 1,
      })),
      hook: ArcShared::new(NoStdMutex::new(None)),
      runtime,
    }
  }

  /// Builds a transport instance for the factory.
  pub(crate) fn build() -> Result<Self, TransportError> {
    Ok(Self::new())
  }

  fn encode_frame(payload: &[u8], correlation_id: CorrelationId) -> Vec<u8> {
    let header_len = 12_usize;
    let total = header_len + payload.len();
    let mut frame = Vec::with_capacity(4 + total);
    frame.extend_from_slice(&(total as u32).to_be_bytes());
    frame.extend_from_slice(&correlation_id.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
  }

  fn fire_backpressure(&self, authority: &str, signal: BackpressureSignal, correlation_id: CorrelationId) {
    if let Some(hook) = self.hook.lock().clone() {
      hook.on_backpressure(signal, authority, correlation_id);
    }
  }

  async fn accept_loop(
    listener: TcpListener,
    authority: String,
    hook: ArcShared<NoStdMutex<Option<ArcShared<dyn TransportBackpressureHook>>>>,
  ) {
    loop {
      match listener.accept().await {
        | Ok((stream, _peer)) => {
          let authority_clone = authority.clone();
          let hook_clone = hook.clone();
          tokio::spawn(async move {
            if let Err(e) = Self::handle_inbound(stream, &authority_clone, hook_clone).await {
              eprintln!("Inbound connection error: {e:?}");
            }
          });
        },
        | Err(e) => {
          eprintln!("Accept error: {e:?}");
          break;
        },
      }
    }
  }

  async fn handle_inbound(
    mut stream: TcpStream,
    authority: &str,
    hook: ArcShared<NoStdMutex<Option<ArcShared<dyn TransportBackpressureHook>>>>,
  ) -> Result<(), TransportError> {
    let mut buffer = Vec::new();
    loop {
      let mut len_bytes = [0u8; 4];
      if stream.read_exact(&mut len_bytes).await.is_err() {
        break;
      }
      let total_len = u32::from_be_bytes(len_bytes) as usize;
      if total_len < 12 {
        return Err(TransportError::Io("invalid frame: length too short".into()));
      }
      buffer.resize(total_len, 0);
      stream.read_exact(&mut buffer).await.map_err(|e| TransportError::Io(format!("frame read failed: {e}")))?;
      // CorrelationId は 96bit (12 bytes) = u64 (8 bytes) + u32 (4 bytes)
      let hi = u64::from_be_bytes(buffer[0..8].try_into().unwrap());
      let lo = u32::from_be_bytes(buffer[8..12].try_into().unwrap());
      let correlation_id = CorrelationId::new(hi, lo);
      let _payload = &buffer[12..];
      if let Some(hook_ref) = hook.lock().clone() {
        hook_ref.on_backpressure(BackpressureSignal::Release, authority, correlation_id);
      }
    }
    Ok(())
  }

  async fn sender_loop(
    mut stream: TcpStream,
    authority: String,
    mut receiver: mpsc::Receiver<OutboundFrame>,
    hook: ArcShared<NoStdMutex<Option<ArcShared<dyn TransportBackpressureHook>>>>,
  ) {
    let mut pending_count = 0_usize;
    while let Some(frame) = receiver.recv().await {
      let encoded = Self::encode_frame(&frame.payload, frame.correlation_id);
      if stream.write_all(&encoded).await.is_err() {
        break;
      }
      pending_count += 1;
      if pending_count >= BACKPRESSURE_THRESHOLD {
        if let Some(hook_ref) = hook.lock().clone() {
          hook_ref.on_backpressure(BackpressureSignal::Apply, &authority, frame.correlation_id);
        }
      }
    }
  }
}

impl RemoteTransport for TokioTcpTransport {
  fn scheme(&self) -> &str {
    "fraktor.tcp"
  }

  fn spawn_listener(&self, bind: &TransportBind) -> Result<TransportHandle, TransportError> {
    let authority = bind.authority().to_string();
    let hook = self.hook.clone();

    let listener = self
      .runtime
      .block_on(async { TcpListener::bind(&authority).await })
      .map_err(|e| TransportError::Io(format!("bind failed: {e}")))?;

    let task = self.runtime.spawn(Self::accept_loop(listener, authority.clone(), hook));

    let mut guard = self.state.lock();
    guard.listeners.insert(authority.clone(), ListenerHandle { _task: task });

    Ok(TransportHandle::new(&authority))
  }

  fn open_channel(&self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError> {
    let authority = endpoint.authority().to_string();
    let hook = self.hook.clone();

    let stream = self
      .runtime
      .block_on(async { TcpStream::connect(&authority).await })
      .map_err(|e| TransportError::Io(format!("connection failed: {e}")))?;

    let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);

    self.runtime.spawn(Self::sender_loop(stream, authority.clone(), receiver, hook));

    let mut guard = self.state.lock();
    let id = guard.next_channel;
    guard.next_channel += 1;
    guard.channels.insert(id, ChannelHandle { authority, sender });

    Ok(TransportChannel::new(id))
  }

  fn send(
    &self,
    channel: &TransportChannel,
    payload: &[u8],
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError> {
    let guard = self.state.lock();
    let handle = guard.channels.get(&channel.id()).ok_or(TransportError::ChannelUnavailable(channel.id()))?;

    let frame = OutboundFrame { payload: payload.to_vec(), correlation_id };

    handle.sender.try_send(frame).map_err(|_| TransportError::ChannelUnavailable(channel.id()))?;

    Ok(())
  }

  fn close(&self, channel: &TransportChannel) {
    self.state.lock().channels.remove(&channel.id());
  }

  fn install_backpressure_hook(&self, hook: ArcShared<dyn TransportBackpressureHook>) {
    *self.hook.lock() = Some(hook);
  }
}
