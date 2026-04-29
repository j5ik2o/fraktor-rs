//! Adapter sender that preserves remote actor identity until message
//! serialization is wired.

use fraktor_actor_core_rs::core::kernel::actor::{
  actor_ref::{ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};
use fraktor_remote_core_rs::core::provider::RemoteActorRef;

/// Sender that wraps a [`RemoteActorRef`] together with a shared
/// canonical path.
///
/// Phase 2 restores remote references for identity-bearing messages and
/// routee expansion, but user payload serialization remains a Phase 3
/// contract. Sending therefore fails synchronously instead of handing an
/// unserialized message to the TCP transport and producing an empty payload
/// frame.
pub struct RemoteActorRefSender {
  remote_ref: RemoteActorRef,
}

impl RemoteActorRefSender {
  /// Creates a new sender for the given `remote_ref`.
  #[must_use]
  pub const fn new(remote_ref: RemoteActorRef) -> Self {
    Self { remote_ref }
  }

  /// Returns the wrapped [`RemoteActorRef`].
  #[must_use]
  pub const fn remote_ref(&self) -> &RemoteActorRef {
    &self.remote_ref
  }
}

impl ActorRefSender for RemoteActorRefSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::invalid_payload(message, "remote payload serialization is not installed"))
  }
}
