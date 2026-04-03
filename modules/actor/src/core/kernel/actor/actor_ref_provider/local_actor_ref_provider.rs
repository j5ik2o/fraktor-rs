//! Local-only actor reference provider.

use alloc::{format, string::String};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathParts, ActorPathScheme, GuardianKind},
    actor_ref::{ActorRef, ActorRefSender, SendOutcome, dead_letter::DeadLetterReason},
    actor_ref_provider::ActorRefProvider,
    error::{ActorError, SendError},
    messaging::AnyMessage,
  },
  system::state::{SystemStateShared, SystemStateWeak},
};

#[cfg(test)]
mod tests;

// 他の typed/system facade 用セントネル (`u64::MAX`, `u64::MAX - 1`,
// `u64::MAX - 2`) に隣接する高い PID を予約し、この provider スコープの
// dead-letter facade がランタイム割り当ての actor PID と衝突しないようにする。
const PROVIDER_DEAD_LETTER_PID: Pid = Pid::new(u64::MAX - 3, 0);

struct ProviderDeadLetterSender {
  state: SystemStateWeak,
}

impl ProviderDeadLetterSender {
  const fn new(state: SystemStateWeak) -> Self {
    Self { state }
  }
}

impl ActorRefSender for ProviderDeadLetterSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let Some(state) = self.state.upgrade() else {
      return Err(SendError::closed(message));
    };
    state.record_dead_letter(message, DeadLetterReason::ExplicitRouting, None);
    Ok(SendOutcome::Delivered)
  }
}

/// Provider for local-only actor systems.
///
/// This provider only supports local actor references and will return an error
/// if asked to create a reference for a remote actor path (with authority).
pub struct LocalActorRefProvider {
  state: Option<SystemStateWeak>,
}

impl LocalActorRefProvider {
  /// Creates a new local actor reference provider.
  #[must_use]
  pub const fn new() -> Self {
    Self { state: None }
  }

  /// Creates a local actor reference provider bound to the provided system state.
  #[must_use]
  pub fn new_with_state(state: &SystemStateShared) -> Self {
    Self { state: Some(state.downgrade()) }
  }

  fn state(&self) -> Option<SystemStateShared> {
    self.state.as_ref()?.upgrade()
  }

  fn guardian_ref(&self, guardian: GuardianKind) -> Option<ActorRef> {
    let state = self.state()?;
    let pid = match guardian {
      | GuardianKind::System => state.system_guardian_pid(),
      | GuardianKind::User => state.user_guardian_pid(),
    }?;
    state.cell(&pid).map(|cell| cell.actor_ref())
  }

  fn resolve_local_path(&self, path: &ActorPath) -> Result<ActorRef, ActorError> {
    let Some(state) = self.state() else {
      return Err(ActorError::fatal("LocalActorRefProvider is not bound to a system state"));
    };

    let segments = path.segments();
    match segments {
      | [guardian] if guardian.as_str() == GuardianKind::User.segment() => {
        return self.guardian().ok_or_else(|| ActorError::fatal("user guardian is not available"));
      },
      | [guardian] if guardian.as_str() == GuardianKind::System.segment() => {
        return self.system_guardian().ok_or_else(|| ActorError::fatal("system guardian is not available"));
      },
      | [guardian, temp, name] if guardian.as_str() == GuardianKind::User.segment() && temp.as_str() == "temp" => {
        return state
          .temp_actor(name.as_str())
          .ok_or_else(|| ActorError::fatal(format!("temporary actor not found: {}", name.as_str())));
      },
      | _ => {},
    }

    let Some(pid) = state.with_actor_path_registry(|registry| registry.pid_for(path)) else {
      return Err(ActorError::fatal(format!("actor path not found: {}", path.to_relative_string())));
    };

    state
      .cell(&pid)
      .map(|cell| cell.actor_ref())
      .ok_or_else(|| ActorError::fatal(format!("actor cell not found for pid {:?}", pid)))
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

    self.resolve_local_path(&path)
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    let state = self.state()?;
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
    let Some(state) = self.state() else {
      debug_assert!(false, "LocalActorRefProvider.state not initialized");
      return ActorRef::null();
    };
    ActorRef::with_system(PROVIDER_DEAD_LETTER_PID, ProviderDeadLetterSender::new(state.downgrade()), &state)
  }

  fn root_path(&self) -> ActorPath {
    let Some(state) = self.state() else {
      return ActorPath::from_parts(ActorPathParts::local("cellactor").with_guardian(GuardianKind::User));
    };
    ActorPath::from_parts(ActorPathParts::local(state.system_name()).with_guardian(GuardianKind::User))
  }

  fn resolve_actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.actor_ref(path)
  }

  fn register_temp_actor(&self, actor: ActorRef) -> Option<String> {
    let state = self.state()?;
    Some(state.register_temp_actor(actor))
  }

  fn unregister_temp_actor(&self, name: &str) {
    if let Some(state) = self.state() {
      state.unregister_temp_actor(name);
    }
  }

  fn temp_actor(&self, name: &str) -> Option<ActorRef> {
    self.state()?.temp_actor(name)
  }
}
