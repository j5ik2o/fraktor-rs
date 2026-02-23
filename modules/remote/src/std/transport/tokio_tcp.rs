//! Tokio TCP transport for production remoting.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::fmt;
use std::net::{TcpListener as StdTcpListener, TcpStream as StdTcpStream};

use fraktor_actor_rs::core::event::stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::{
  core::{
    runtime_toolbox::NoStdMutex,
    sync::{ArcShared, SharedAccess},
  },
  std::runtime_toolbox::StdToolbox,
};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener as TokioTcpListener, TcpStream as TokioTcpStream},
  runtime::{Builder, Handle, Runtime},
  sync::mpsc,
  task::JoinHandle,
};

use crate::core::transport::{
  RemoteTransport, TransportBackpressureHookShared, TransportBind, TransportChannel, TransportEndpoint, TransportError,
  TransportHandle,
  inbound::{InboundFrame, TransportInboundShared},
};

const BACKPRESSURE_THRESHOLD: usize = 1024;
const CHANNEL_BUFFER_SIZE: usize = 256;

/// Tokio-based TCP transport implementing the Pekko wire protocol.
///
/// This transport is specialized for [`StdToolbox`] because it uses Tokio's async runtime
/// and requires `Send + Sync` bounds on the inbound handler that are only available
/// with standard library mutex implementations.
pub struct TokioTcpTransport {
  state:    TokioTcpState,
  // hook と inbound は非同期タスクとの共有のため Arc<Mutex> を維持
  hook:     ArcShared<NoStdMutex<Option<TransportBackpressureHookShared>>>,
  inbound:  ArcShared<NoStdMutex<Option<TransportInboundShared<StdToolbox>>>>,
  handle:   Handle,
  _runtime: Option<Runtime>,
}

struct TokioTcpState {
  listeners:    BTreeMap<String, ListenerHandle>,
  channels:     BTreeMap<u64, ChannelHandle>,
  next_channel: u64,
}

struct ListenerHandle {
  _task: JoinHandle<()>,
}

struct ChannelHandle {
  #[allow(dead_code)]
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
  /// Creates a new Tokio TCP transport backed by an internal Tokio runtime.
  #[must_use]
  pub fn new() -> Self {
    Self::try_new().expect("tokio runtime unavailable")
  }

  /// Attempts to create a new transport instance, returning a transport error on failure.
  pub fn try_new() -> Result<Self, TransportError> {
    if let Ok(handle) = Handle::try_current() {
      return Ok(Self::with_handle(handle));
    }

    let runtime = Builder::new_multi_thread()
      .enable_time()
      .enable_io()
      .build()
      .map_err(|error| TransportError::Io(format!("failed to build tokio runtime: {error}")))?;
    Ok(Self::with_runtime(runtime))
  }

  /// Creates a new Tokio TCP transport using the provided runtime.
  pub fn with_runtime(runtime: Runtime) -> Self {
    Self {
      state:    TokioTcpState { listeners: BTreeMap::new(), channels: BTreeMap::new(), next_channel: 1 },
      hook:     ArcShared::new(NoStdMutex::new(None)),
      inbound:  ArcShared::new(NoStdMutex::new(None)),
      handle:   runtime.handle().clone(),
      _runtime: Some(runtime),
    }
  }

  /// Creates a new Tokio TCP transport using an existing Tokio runtime handle.
  pub fn with_handle(handle: Handle) -> Self {
    Self {
      state: TokioTcpState { listeners: BTreeMap::new(), channels: BTreeMap::new(), next_channel: 1 },
      hook: ArcShared::new(NoStdMutex::new(None)),
      inbound: ArcShared::new(NoStdMutex::new(None)),
      handle,
      _runtime: None,
    }
  }

  /// Builds a transport instance for the factory.
  pub(crate) fn build() -> Result<Self, TransportError> {
    Self::try_new()
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

  #[allow(dead_code)]
  fn fire_backpressure(&self, authority: &str, signal: BackpressureSignal, correlation_id: CorrelationId) {
    if let Some(hook) = self.hook.lock().clone() {
      hook.with_write(|h| h.on_backpressure(signal, authority, correlation_id));
    }
  }

  async fn accept_loop(
    listener: TokioTcpListener,
    authority: String,
    hook: ArcShared<NoStdMutex<Option<TransportBackpressureHookShared>>>,
    inbound: ArcShared<NoStdMutex<Option<TransportInboundShared<StdToolbox>>>>,
  ) {
    loop {
      match listener.accept().await {
        | Ok((stream, peer)) => {
          let authority_clone = authority.clone();
          let hook_clone = hook.clone();
          let inbound_clone = inbound.clone();
          let remote = peer.to_string();
          tokio::spawn(async move {
            if let Err(e) = Self::handle_inbound(stream, authority_clone, remote, hook_clone, inbound_clone).await {
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
    mut stream: TokioTcpStream,
    authority: String,
    remote: String,
    hook: ArcShared<NoStdMutex<Option<TransportBackpressureHookShared>>>,
    inbound: ArcShared<NoStdMutex<Option<TransportInboundShared<StdToolbox>>>>,
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
      const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;
      if total_len > MAX_FRAME_SIZE {
        return Err(TransportError::Io("invalid frame: length exceeds maximum (16 MiB)".into()));
      }
      buffer.resize(total_len, 0);
      stream.read_exact(&mut buffer).await.map_err(|e| TransportError::Io(format!("frame read failed: {e}")))?;
      // CorrelationId は 96bit (12 bytes) = u64 (8 bytes) + u32 (4 bytes)
      let hi = u64::from_be_bytes(buffer[0..8].try_into().unwrap());
      let lo = u32::from_be_bytes(buffer[8..12].try_into().unwrap());
      let correlation_id = CorrelationId::new(hi, lo);
      let _payload = &buffer[12..];
      if let Some(hook_ref) = hook.lock().clone() {
        hook_ref.with_write(|h| h.on_backpressure(BackpressureSignal::Release, &authority, correlation_id));
      }
      if let Some(handler) = inbound.lock().clone() {
        handler.with_write(|h| {
          h.on_frame(InboundFrame::new(&authority, remote.clone(), buffer[12..].to_vec(), correlation_id))
        });
      }
    }
    Ok(())
  }

  async fn sender_loop(
    mut stream: TokioTcpStream,
    authority: String,
    mut receiver: mpsc::Receiver<OutboundFrame>,
    hook: ArcShared<NoStdMutex<Option<TransportBackpressureHookShared>>>,
  ) {
    let mut pending_count = 0_usize;
    while let Some(frame) = receiver.recv().await {
      let encoded = Self::encode_frame(&frame.payload, frame.correlation_id);
      if stream.write_all(&encoded).await.is_err() {
        break;
      }
      pending_count += 1;
      if pending_count >= BACKPRESSURE_THRESHOLD
        && let Some(hook_ref) = hook.lock().clone()
      {
        hook_ref.with_write(|h| h.on_backpressure(BackpressureSignal::Apply, &authority, frame.correlation_id));
      }
    }
  }
}

impl RemoteTransport<StdToolbox> for TokioTcpTransport {
  fn scheme(&self) -> &str {
    "fraktor.tcp"
  }

  fn spawn_listener(&mut self, bind: &TransportBind) -> Result<TransportHandle, TransportError> {
    let authority = bind.authority().to_string();
    let hook = self.hook.clone();
    let inbound = self.inbound.clone();

    let authority_clone = authority.clone();
    let std_listener =
      StdTcpListener::bind(&authority_clone).map_err(|e| TransportError::Io(format!("bind failed: {e}")))?;
    std_listener
      .set_nonblocking(true)
      .map_err(|e| TransportError::Io(format!("failed to configure non-blocking listener: {e}")))?;
    let listener = {
      let _enter = self.handle.enter();
      TokioTcpListener::from_std(std_listener).map_err(|e| TransportError::Io(format!("bind failed: {e}")))?
    };

    let task = self.handle.spawn(Self::accept_loop(listener, authority.clone(), hook, inbound));

    self.state.listeners.insert(authority.clone(), ListenerHandle { _task: task });

    Ok(TransportHandle::new(&authority))
  }

  fn open_channel(&mut self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError> {
    let authority = endpoint.authority().to_string();
    let hook = self.hook.clone();

    let authority_clone = authority.clone();
    let std_stream =
      StdTcpStream::connect(&authority_clone).map_err(|e| TransportError::Io(format!("connection failed: {e}")))?;
    std_stream
      .set_nonblocking(true)
      .map_err(|e| TransportError::Io(format!("failed to configure non-blocking stream: {e}")))?;
    let stream = {
      let _enter = self.handle.enter();
      TokioTcpStream::from_std(std_stream).map_err(|e| TransportError::Io(format!("connection failed: {e}")))?
    };

    let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);

    self.handle.spawn(Self::sender_loop(stream, authority.clone(), receiver, hook));

    let id = self.state.next_channel;
    self.state.next_channel += 1;
    self.state.channels.insert(id, ChannelHandle { authority, sender });

    Ok(TransportChannel::new(id))
  }

  fn send(
    &mut self,
    channel: &TransportChannel,
    payload: &[u8],
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError> {
    let handle = self.state.channels.get(&channel.id()).ok_or(TransportError::ChannelUnavailable(channel.id()))?;

    let frame = OutboundFrame { payload: payload.to_vec(), correlation_id };

    handle.sender.try_send(frame).map_err(|_| TransportError::ChannelUnavailable(channel.id()))?;

    Ok(())
  }

  fn close(&mut self, channel: &TransportChannel) {
    self.state.channels.remove(&channel.id());
  }

  fn install_backpressure_hook(&mut self, hook: TransportBackpressureHookShared) {
    *self.hook.lock() = Some(hook);
  }

  fn install_inbound_handler(&mut self, handler: TransportInboundShared<StdToolbox>) {
    *self.inbound.lock() = Some(handler);
  }
}
