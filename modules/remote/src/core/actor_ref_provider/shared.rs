use alloc::format;

use fraktor_actor_rs::core::kernel::{
  actor::{
    Address, Pid,
    actor_path::{ActorPath, ActorPathParts, ActorPathScheme, GuardianKind},
    actor_ref::{ActorRef, NullSender},
    deploy::Deployer,
    error::ActorError,
  },
  system::{ActorSystemWeak, TerminationSignal, state::SystemStateShared},
};

use super::remote_error::RemoteActorRefProviderError;

/// Reserved PID in the high sentinel range to avoid collisions with local provider sentinels.
pub(crate) const PROVIDER_TEMP_CONTAINER_PID: Pid = Pid::new(u64::MAX - 5, 0);

pub(crate) trait SharedRemoteActorRefProvider {
  fn actor_system_weak(&self) -> &ActorSystemWeak;
  fn create_remote_actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, RemoteActorRefProviderError>;
  fn map_actor_ref_error(error: RemoteActorRefProviderError) -> ActorError;
  fn system_unavailable_message() -> &'static str;

  fn provider_state(&self) -> Option<SystemStateShared> {
    self.actor_system_weak().upgrade().map(|system| system.state())
  }

  fn default_address_from_state(state: &SystemStateShared) -> Option<Address> {
    match state.canonical_authority_components() {
      | Some((host, Some(port))) => Some(Address::remote(state.system_name(), host, port)),
      | _ => Some(Address::local(state.system_name())),
    }
  }

  fn root_path_for_state(state: &SystemStateShared) -> ActorPath {
    ActorPath::from_parts(ActorPathParts::local(state.system_name()).with_guardian(GuardianKind::User))
  }

  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::FraktorTcp]
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.create_remote_actor_ref(path).map_err(Self::map_actor_ref_error)
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    self.provider_state()?.root_guardian().map(|cell| cell.actor_ref())
  }

  fn guardian(&self) -> Option<ActorRef> {
    self.provider_state()?.user_guardian().map(|cell| cell.actor_ref())
  }

  fn system_guardian(&self) -> Option<ActorRef> {
    self.provider_state()?.system_guardian().map(|cell| cell.actor_ref())
  }

  fn root_path(&self) -> ActorPath {
    self.provider_state().map_or_else(
      || ActorPath::from_parts(ActorPathParts::local("cellactor").with_guardian(GuardianKind::User)),
      |state| Self::root_path_for_state(&state),
    )
  }

  fn root_guardian_at(&self, address: &Address) -> Option<ActorRef> {
    let default = self.get_default_address()?;
    if (!address.has_global_scope() && address.system() == default.system()) || *address == default {
      self.root_guardian()
    } else {
      None
    }
  }

  fn deployer(&self) -> Option<Deployer> {
    Some(self.provider_state()?.deployer())
  }

  fn temp_path(&self) -> ActorPath {
    self.root_path().child("temp")
  }

  fn temp_path_with_prefix(&self, prefix: &str) -> Result<ActorPath, ActorError> {
    let state = self.provider_state().ok_or_else(|| ActorError::fatal(Self::system_unavailable_message()))?;
    let generated = if prefix.is_empty() {
      state.next_temp_actor_name_with_prefix("tmp")
    } else {
      state.next_temp_actor_name_with_prefix(prefix)
    };
    self
      .temp_path()
      .try_child(&generated)
      .map_err(|error| ActorError::fatal(alloc::format!("invalid temp path: {error}")))
  }

  /// Returns the `/temp` container `ActorRef`.
  ///
  /// Each call re-registers the provider temp path in the actor path registry,
  /// so callers should cache the returned ref when they need repeated access.
  fn temp_container(&self) -> Option<ActorRef> {
    let state = self.provider_state()?;
    state.register_actor_path(PROVIDER_TEMP_CONTAINER_PID, &self.temp_path());
    Some(ActorRef::with_system(PROVIDER_TEMP_CONTAINER_PID, NullSender, &state))
  }

  fn register_temp_actor(&self, actor: ActorRef) -> Option<alloc::string::String> {
    Some(self.provider_state()?.register_temp_actor(actor))
  }

  fn unregister_temp_actor(&self, name: &str) {
    if let Some(state) = self.provider_state() {
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
    self.provider_state()?.temp_actor(name)
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.provider_state().map_or_else(TerminationSignal::already_terminated, |state| state.termination_signal())
  }

  fn get_external_address_for(&self, addr: &Address) -> Option<Address> {
    let default = self.get_default_address()?;
    if (!addr.has_global_scope() && addr.system() == default.system()) || *addr == default {
      Some(default)
    } else {
      None
    }
  }

  fn get_default_address(&self) -> Option<Address> {
    Self::default_address_from_state(&self.provider_state()?)
  }
}
