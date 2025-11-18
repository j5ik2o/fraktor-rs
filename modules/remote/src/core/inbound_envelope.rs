//! Deserialized inbound envelope produced by the endpoint reader.

use fraktor_actor_rs::core::{actor_prim::actor_path::ActorPathParts, messaging::AnyMessageGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::endpoint_manager::RemoteNodeId;

/// Message reconstructed from a remoting frame.
pub struct InboundEnvelope<TB: RuntimeToolbox + 'static> {
  target:   ActorPathParts,
  remote:   RemoteNodeId,
  message:  AnyMessageGeneric<TB>,
  reply_to: Option<ActorPathParts>,
}

impl<TB: RuntimeToolbox + 'static> InboundEnvelope<TB> {
  /// Creates a new inbound envelope from decoded components.
  pub(crate) fn new(
    target: ActorPathParts,
    remote: RemoteNodeId,
    message: AnyMessageGeneric<TB>,
    reply_to: Option<ActorPathParts>,
  ) -> Self {
    Self { target, remote, message, reply_to }
  }

  /// Returns the message recipient metadata.
  #[must_use]
  pub fn target(&self) -> &ActorPathParts {
    &self.target
  }

  /// Returns the remote node metadata.
  #[must_use]
  pub fn remote(&self) -> &RemoteNodeId {
    &self.remote
  }

  /// Returns the deserialized message payload.
  #[must_use]
  pub fn message(&self) -> &AnyMessageGeneric<TB> {
    &self.message
  }

  /// Returns the reply-to metadata if present.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorPathParts> {
    self.reply_to.as_ref()
  }
}
