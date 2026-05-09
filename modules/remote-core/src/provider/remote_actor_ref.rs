//! Data-only remote actor reference returned by the provider.

use fraktor_actor_core_kernel_rs::actor::actor_path::ActorPath;

use crate::address::RemoteNodeId;

/// Data-only handle describing a remote actor.
///
/// This type deliberately has **no** `send` / `tell` / `ask` methods: it is
/// merely the result of resolving an [`ActorPath`] through a
/// [`crate::provider::RemoteActorRefProvider`]. Actual message delivery
/// happens in Phase B, where the adapter layer pairs a `RemoteActorRef` with
/// an [`crate::envelope::OutboundEnvelope`] and hands it to the
/// [`crate::association::Association`] state machine.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RemoteActorRef {
  path:        ActorPath,
  remote_node: RemoteNodeId,
}

impl RemoteActorRef {
  /// Creates a new [`RemoteActorRef`].
  #[must_use]
  pub const fn new(path: ActorPath, remote_node: RemoteNodeId) -> Self {
    Self { path, remote_node }
  }

  /// Returns the remote actor path.
  #[must_use]
  pub const fn path(&self) -> &ActorPath {
    &self.path
  }

  /// Returns the remote node identity.
  #[must_use]
  pub const fn remote_node(&self) -> &RemoteNodeId {
    &self.remote_node
  }
}
