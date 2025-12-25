//! Inspects actor reference fields before sending and rejects quarantined authorities.

use alloc::string::{String, ToString};

use fraktor_actor_rs::core::{
  actor::actor_path::ActorPath,
  event::stream::{EventStreamEvent, RemoteAuthorityEvent},
  system::{AuthorityState, RemoteAuthorityError, SystemStateSharedGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

/// Helper that validates actor reference fields inside messages.
pub(crate) struct ActorRefFieldNormalizerGeneric<TB: RuntimeToolbox + 'static> {
  system_state: SystemStateSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> ActorRefFieldNormalizerGeneric<TB> {
  /// Creates a new normalizer.
  pub(crate) fn new(system_state: SystemStateSharedGeneric<TB>) -> Self {
    Self { system_state }
  }

  /// Validates the recipient path for quarantined authority.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] when the authority is quarantined.
  pub(crate) fn validate_recipient(&self, recipient: &ActorPath) -> Result<(), RemoteAuthorityError> {
    if let Some(authority) = recipient.parts().authority_endpoint() {
      self.reject_if_quarantined(&authority)?;
    }
    Ok(())
  }

  /// Validates sender and rejects quarantined authority.
  pub(crate) fn validate_sender(
    &self,
    message: &fraktor_actor_rs::core::messaging::AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    if let Some(sender) = message.sender()
      && let Some(path) = sender.canonical_path()
      && let Some(authority) = path.parts().authority_endpoint()
    {
      self.reject_if_quarantined(&authority)?;
    }
    Ok(())
  }

  fn reject_if_quarantined(&self, authority: &str) -> Result<(), RemoteAuthorityError> {
    let state = self.system_state.remote_authority_state(authority);
    if matches!(state, AuthorityState::Quarantine { .. }) {
      self.publish_remote_event(authority.to_string(), state);
      return Err(RemoteAuthorityError::Quarantined);
    }
    Ok(())
  }

  fn publish_remote_event(&self, authority: String, state: AuthorityState) {
    let event = RemoteAuthorityEvent::new(authority, state);
    self.system_state.event_stream().publish(&EventStreamEvent::RemoteAuthority(event));
  }
}
