//! Adapter sender that bridges actor-core's `ActorRefSender` trait to a
//! `TcpRemoteTransport`.

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Pid,
    actor_path::ActorPath,
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  event::stream::CorrelationId,
};
use fraktor_remote_core_rs::{
  address::RemoteNodeId,
  envelope::{OutboundEnvelope, OutboundPriority},
  provider::RemoteActorRef,
  transport::RemoteTransport,
};
use fraktor_utils_core_rs::core::sync::SharedLock;

use crate::tcp_transport::TcpRemoteTransport;

/// Sender that wraps a [`RemoteActorRef`] together with a shared
/// `TcpRemoteTransport` and forwards every actor-core `send` call into the
/// transport's `RemoteTransport::send` entry point.
///
/// This is the bridge that lets actor-core consumers transparently message
/// remote actors: they call `ActorRef::tell(message)` exactly as if the
/// recipient were local, while the underlying sender packages the
/// `AnyMessage` into an [`OutboundEnvelope`] and hands it to the TCP
/// transport.
///
/// Phase B minimum-viable: the message payload is **not** serialised here
/// (`OutboundEnvelope` carries the `AnyMessage` directly through the
/// `Association` state machine). The actual byte-level encoding happens in
/// `TcpRemoteTransport::send`, which currently writes a placeholder
/// envelope frame; full payload serialisation is the responsibility of the
/// `serialization` extension and is out of scope for Phase B.
pub struct RemoteActorRefSender {
  remote_ref:  RemoteActorRef,
  transport:   SharedLock<TcpRemoteTransport>,
  #[allow(dead_code)]
  watcher_pid: Pid,
}

impl RemoteActorRefSender {
  /// Creates a new sender for the given `remote_ref`, wired to `transport`.
  ///
  /// `watcher_pid` records which local Pid owns the reference, mirroring
  /// Pekko's `RemoteActorRef.watcherActor`. The field is currently a
  /// placeholder for the death-watch wiring added in Section 22.
  #[must_use]
  pub fn new(remote_ref: RemoteActorRef, transport: SharedLock<TcpRemoteTransport>, watcher_pid: Pid) -> Self {
    Self { remote_ref, transport, watcher_pid }
  }

  /// Returns the wrapped [`RemoteActorRef`].
  #[must_use]
  pub const fn remote_ref(&self) -> &RemoteActorRef {
    &self.remote_ref
  }
}

impl ActorRefSender for RemoteActorRefSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let path = self.remote_ref.path().clone();
    let node = self.remote_ref.remote_node().clone();
    let envelope = build_envelope_for(path, node, message);
    self
      .transport
      .with_lock(|transport| transport.send(envelope))
      .map(|_| SendOutcome::Delivered)
      .map_err(|_| SendError::closed(AnyMessage::new(())))
  }
}

fn build_envelope_for(recipient: ActorPath, remote_node: RemoteNodeId, message: AnyMessage) -> OutboundEnvelope {
  OutboundEnvelope::new(recipient, None, message, OutboundPriority::User, remote_node, CorrelationId::nil())
}
