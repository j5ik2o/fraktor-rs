//! Local-only actor reference provider.

use crate::core::kernel::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathScheme, GuardianKind},
    actor_ref::{ActorRef, ActorRefSender, SendOutcome, dead_letter::DeadLetterReason},
    actor_ref_provider::ActorRefProvider,
    error::{ActorError, SendError},
    messaging::AnyMessage,
  },
  system::state::SystemStateShared,
};

#[cfg(test)]
mod tests;

const PROVIDER_DEAD_LETTER_PID: Pid = Pid::new(u64::MAX - 3, 0);

struct ProviderDeadLetterSender {
  state: SystemStateShared,
}

impl ProviderDeadLetterSender {
  const fn new(state: SystemStateShared) -> Self {
    Self { state }
  }
}

impl ActorRefSender for ProviderDeadLetterSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.state.record_dead_letter(message, DeadLetterReason::ExplicitRouting, None);
    Ok(SendOutcome::Delivered)
  }
}

/// Provider for local-only actor systems.
///
/// This provider only supports local actor references and will return an error
/// if asked to create a reference for a remote actor path (with authority).
pub struct LocalActorRefProvider {
  state: Option<SystemStateShared>,
}

impl LocalActorRefProvider {
  /// Creates a new local actor reference provider.
  #[must_use]
  pub const fn new() -> Self {
    Self { state: None }
  }

  /// Creates a local actor reference provider bound to the provided system state.
  #[must_use]
  pub const fn new_with_state(state: SystemStateShared) -> Self {
    Self { state: Some(state) }
  }

  fn guardian_ref(&self, guardian: GuardianKind) -> Option<ActorRef> {
    let state = self.state.as_ref()?;
    let pid = match guardian {
      | GuardianKind::System => state.system_guardian_pid(),
      | GuardianKind::User => state.user_guardian_pid(),
    }?;
    state.cell(&pid).map(|cell| cell.actor_ref())
  }
}

impl Default for LocalActorRefProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl ActorRefProvider for LocalActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::Fraktor]
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    // Local provider only supports local paths (no authority)
    if path.parts().authority_endpoint().is_some() {
      return Err(ActorError::fatal("LocalActorRefProvider does not support remote actor paths"));
    }

    // For local-only systems, actor references are typically created through
    // ActorContext::spawn_child() rather than through the provider.
    // This method is primarily for path-based lookups, which are not yet implemented.
    Err(ActorError::fatal("Path-based actor lookup not yet implemented for local provider"))
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    let state = self.state.as_ref()?;
    let pid = state.root_guardian_pid()?;
    state.cell(&pid).map(|cell| cell.actor_ref())
  }

  fn guardian(&self) -> Option<ActorRef> {
    self.guardian_ref(GuardianKind::User)
  }

  fn system_guardian(&self) -> Option<ActorRef> {
    self.guardian_ref(GuardianKind::System)
  }

  fn dead_letters(&self) -> ActorRef {
    let Some(state) = &self.state else {
      return ActorRef::null();
    };
    ActorRef::with_system(PROVIDER_DEAD_LETTER_PID, ProviderDeadLetterSender::new(state.clone()), state)
  }

  fn temp_path(&self) -> ActorPath {
    ActorPath::root().child("temp")
  }
}
