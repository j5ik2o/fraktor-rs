//! Local-only actor reference provider.

use alloc::{format, string::String};

use crate::{
  actor::{
    Address, Pid,
    actor_path::{ActorPath, ActorPathParts, ActorPathScheme, GuardianKind},
    actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome, dead_letter::DeadLetterReason},
    actor_ref_provider::ActorRefProvider,
    deploy::Deployer,
    error::{ActorError, SendError},
    messaging::AnyMessage,
  },
  system::{
    TerminationSignal,
    state::{SystemStateShared, SystemStateWeak},
  },
};

#[cfg(test)]
#[path = "local_actor_ref_provider_test.rs"]
mod tests;

// 他の typed/system facade 用セントネル (`u64::MAX`, `u64::MAX - 1`,
// `u64::MAX - 2`) に隣接する高い PID を予約し、この provider スコープの
// dead-letter facade がランタイム割り当ての actor PID と衝突しないようにする。
const PROVIDER_DEAD_LETTER_PID: Pid = Pid::new(u64::MAX - 3, 0);
const PROVIDER_TEMP_CONTAINER_PID: Pid = Pid::new(u64::MAX - 4, 0);

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

  fn default_address_from_state(state: &SystemStateShared) -> Address {
    match state.canonical_authority_components() {
      | Some((host, Some(port))) => Address::remote(state.system_name(), host, port),
      | _ => Address::local(state.system_name()),
    }
  }

  fn temp_root_path(&self) -> ActorPath {
    self.root_path().child("temp")
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

  fn root_guardian_at(&self, address: &Address) -> Option<ActorRef> {
    let state = self.state()?;
    let default = Self::default_address_from_state(&state);
    if (!address.has_global_scope() && address.system() == default.system()) || *address == default {
      self.root_guardian()
    } else {
      None
    }
  }

  fn deployer(&self) -> Option<Deployer> {
    Some(self.state()?.deployer())
  }

  fn temp_path(&self) -> ActorPath {
    self.temp_root_path()
  }

  fn temp_path_with_prefix(&self, prefix: &str) -> Result<ActorPath, ActorError> {
    let Some(state) = self.state() else {
      return Err(ActorError::fatal("LocalActorRefProvider is not bound to a system state"));
    };
    let generated = if prefix.is_empty() {
      state.next_temp_actor_name_with_prefix("tmp")
    } else {
      state.next_temp_actor_name_with_prefix(prefix)
    };
    self
      .temp_root_path()
      .try_child(&generated)
      .map_err(|error| ActorError::fatal(format!("invalid temp path: {error}")))
  }

  fn temp_container(&self) -> Option<ActorRef> {
    let state = self.state()?;
    state.register_actor_path(PROVIDER_TEMP_CONTAINER_PID, &self.temp_root_path());
    Some(ActorRef::with_system(PROVIDER_TEMP_CONTAINER_PID, NullSender, &state))
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

  fn unregister_temp_actor_path(&self, path: &ActorPath) -> Result<(), ActorError> {
    match path.segments() {
      | [guardian, temp, name] if guardian.as_str() == "user" && temp.as_str() == "temp" => {
        self.unregister_temp_actor(name.as_str());
        Ok(())
      },
      | _ => Err(ActorError::fatal(format!("invalid temp actor path: {}", path.to_relative_string()))),
    }
  }

  fn temp_actor(&self, name: &str) -> Option<ActorRef> {
    self.state()?.temp_actor(name)
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.state().map_or_else(TerminationSignal::already_terminated, |state| state.termination_signal())
  }

  fn get_external_address_for(&self, addr: &Address) -> Option<Address> {
    let state = self.state()?;
    let default = Self::default_address_from_state(&state);
    if (!addr.has_global_scope() && addr.system() == default.system()) || *addr == default {
      Some(default)
    } else {
      None
    }
  }

  fn get_default_address(&self) -> Option<Address> {
    Some(Self::default_address_from_state(&self.state()?))
  }
}
