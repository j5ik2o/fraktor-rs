//! Provides actor references targeting remote authorities.

mod installer;
#[cfg(test)]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
  vec::Vec,
};

use ahash::RandomState;
use fraktor_actor_rs::core::kernel::{
  actor::{
    Address, Pid,
    actor_path::{ActorPath, ActorPathParts, ActorPathScheme, GuardianKind},
    actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome},
    actor_ref_provider::ActorRefProvider,
    deploy::Deployer,
    error::{ActorError, SendError},
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  system::{
    ActorSystem, ActorSystemWeak,
    remote::{RemoteAuthorityError, RemoteWatchHook},
  },
  util::futures::ActorFutureShared,
};
use fraktor_utils_rs::core::sync::SharedAccess;
use hashbrown::HashMap;
pub use installer::RemoteActorRefProviderInstaller;

use crate::core::{
  actor_ref_field_normalizer::ActorRefFieldNormalizerGeneric,
  actor_ref_provider::{
    loopback_router, loopback_router::LoopbackDeliveryOutcome, remote_error::RemoteActorRefProviderError,
  },
  endpoint_writer::{EndpointWriterError, EndpointWriterShared},
  envelope::{OutboundMessage, OutboundPriority},
  remote_authority_snapshot::RemoteAuthoritySnapshot,
  remote_node_id::RemoteNodeId,
  remoting_extension::{RemotingControl, RemotingControlShared, RemotingError},
  watcher::{RemoteWatcherCommand, RemoteWatcherDaemon},
};

const PROVIDER_TEMP_CONTAINER_PID: Pid = Pid::new(u64::MAX - 4, 0);

/// Provider that creates [`ActorRef`] instances for remote recipients.
///
/// Uses a weak reference to the actor system to avoid circular references,
/// since this provider is registered into the actor system's extensions.
pub struct RemoteActorRefProvider {
  system:         ActorSystemWeak,
  writer:         EndpointWriterShared,
  control:        RemotingControlShared,
  watcher_daemon: ActorRef,
  watch_entries:  HashMap<Pid, RemoteWatchEntry, RandomState>,
}

/// Provider that creates [`ActorRef`] instances for remote recipients.
impl RemoteActorRefProvider {
  /// Creates a remote actor-ref provider installer with loopback routing enabled.
  #[must_use]
  pub fn loopback() -> RemoteActorRefProviderInstaller {
    RemoteActorRefProviderInstaller::loopback()
  }

  /// Creates a remote actor reference for the provided path.
  pub fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, RemoteActorRefProviderError> {
    let system = self.system.upgrade().ok_or(RemoteActorRefProviderError::SystemUnavailable)?;
    self.control.lock().associate(path.parts()).map_err(RemoteActorRefProviderError::from)?;
    let sender = self.sender_for_path(&path)?;
    let pid = system.allocate_pid();
    self.register_remote_entry(pid, path.clone());
    Ok(ActorRef::with_system(pid, sender, &system.state()))
  }

  pub(crate) fn from_components(
    system: ActorSystem,
    writer: EndpointWriterShared,
    control: RemotingControlShared,
  ) -> Result<Self, RemoteActorRefProviderError> {
    let daemon = RemoteWatcherDaemon::spawn(&system, control.clone())?;
    control.lock().register_remote_watcher_daemon(daemon.clone());
    Ok(Self {
      system: system.downgrade(),
      writer,
      control,
      watcher_daemon: daemon,
      watch_entries: HashMap::with_hasher(RandomState::new()),
    })
  }

  fn sender_for_path(&self, path: &ActorPath) -> Result<RemoteActorRefSender, RemoteActorRefProviderError> {
    let (host, port) = Self::parse_authority(path.parts())?;
    let uid = path.uid().map(|uid| uid.value()).unwrap_or(0);
    let remote = RemoteNodeId::new(path.parts().system(), host, port, uid);
    Ok(RemoteActorRefSender { writer: self.writer.clone(), recipient: path.clone(), remote_node: remote })
  }

  fn parse_authority(parts: &ActorPathParts) -> Result<(String, Option<u16>), RemoteActorRefProviderError> {
    let Some(endpoint) = parts.authority_endpoint() else {
      return Err(RemoteActorRefProviderError::MissingAuthority);
    };
    if let Some((host, port_str)) = endpoint.split_once(':') {
      let port =
        port_str.parse::<u16>().map_err(|_| RemoteActorRefProviderError::InvalidAuthority(endpoint.clone()))?;
      Ok((host.to_string(), Some(port)))
    } else {
      Ok((endpoint, None))
    }
  }

  #[cfg(any(test, feature = "test-support"))]
  /// Returns the underlying writer handle (testing helper).
  pub fn writer_for_test(&self) -> EndpointWriterShared {
    self.writer.clone()
  }

  /// Requests an association/watch with the provided remote address.
  pub fn watch_remote(&mut self, parts: ActorPathParts) -> Result<(), RemotingError> {
    if parts.authority_endpoint().is_none() {
      return Err(RemotingError::TransportUnavailable("missing authority".into()));
    }
    self.record_snapshot_from_parts(&parts);
    self.control.lock().associate(&parts)
  }

  /// Returns the latest remote authority snapshots recorded by the control plane.
  #[must_use]
  pub fn connections_snapshot(&self) -> Vec<crate::core::remote_authority_snapshot::RemoteAuthoritySnapshot> {
    self.control.lock().connections_snapshot()
  }

  fn register_remote_entry(&mut self, pid: Pid, path: ActorPath) {
    self.watch_entries.entry(pid).or_insert_with(|| RemoteWatchEntry::new(path.clone()));
    self.record_snapshot_from_parts(path.parts());
  }

  fn record_snapshot_from_parts(&self, parts: &ActorPathParts) {
    let Some(authority) = parts.authority_endpoint() else {
      return;
    };
    let Some(system) = self.system.upgrade() else {
      return;
    };
    let deferred = system.state().remote_authority_deferred_count(&authority) as u32;
    let state = system.state().remote_authority_state(&authority);
    let ticks = system.state().monotonic_now().as_millis() as u64;
    let snapshot = RemoteAuthoritySnapshot::new(authority, state, ticks, deferred);
    self.control.lock().record_authority_snapshot(snapshot);
  }

  fn dispatch_remote_watch(&mut self, command: RemoteWatcherCommand) {
    self.watcher_daemon.tell(AnyMessage::new(command));
  }

  fn track_watch(&mut self, target: Pid, watcher: Pid) -> Option<(ActorPathParts, bool)> {
    self.watch_entries.get_mut(&target).map(|entry| {
      let added = entry.add_watcher(watcher);
      (entry.target_parts(), added)
    })
  }

  fn track_unwatch(&mut self, target: Pid, watcher: Pid) -> Option<(ActorPathParts, bool)> {
    self.watch_entries.get_mut(&target).map(|entry| {
      let removed = entry.remove_watcher(watcher);
      (entry.target_parts(), removed)
    })
  }

  #[cfg(any(test, feature = "test-support"))]
  /// Returns the set of remote PIDs tracked by the provider (test helper).
  pub fn registered_remote_pids_for_test(&self) -> Vec<Pid> {
    self.watch_entries.keys().copied().collect()
  }

  #[cfg(any(test, feature = "test-support"))]
  /// Returns the watchers registered for a remote PID (test helper).
  pub fn remote_watchers_for_test(&self, pid: Pid) -> Option<Vec<Pid>> {
    self.watch_entries.get(&pid).map(|entry| entry.watchers().to_vec())
  }

  fn state(&self) -> Option<fraktor_actor_rs::core::kernel::system::state::SystemStateShared> {
    self.system.upgrade().map(|system| system.state())
  }

  fn default_address_from_state(
    state: &fraktor_actor_rs::core::kernel::system::state::SystemStateShared,
  ) -> Option<Address> {
    let (host, Some(port)) = state.canonical_authority_components()? else {
      return None;
    };
    Some(Address::remote(state.system_name(), host, port))
  }

  fn root_path_for_state(state: &fraktor_actor_rs::core::kernel::system::state::SystemStateShared) -> ActorPath {
    ActorPath::from_parts(ActorPathParts::local(state.system_name()).with_guardian(GuardianKind::User))
  }
}

impl RemoteWatchHook for RemoteActorRefProvider {
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    if let Some((parts, should_send)) = self.track_watch(target, watcher) {
      if should_send {
        self.dispatch_remote_watch(RemoteWatcherCommand::Watch { target: parts, watcher });
      }
      true
    } else {
      false
    }
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    if let Some((parts, removed)) = self.track_unwatch(target, watcher) {
      if removed {
        self.dispatch_remote_watch(RemoteWatcherCommand::Unwatch { target: parts, watcher });
      }
      true
    } else {
      false
    }
  }
}

struct RemoteActorRefSender {
  writer:      EndpointWriterShared,
  recipient:   ActorPath,
  remote_node: RemoteNodeId,
}

impl RemoteActorRefSender {
  fn determine_priority(message: &AnyMessage) -> OutboundPriority {
    if message.as_view().downcast_ref::<SystemMessage>().is_some() {
      OutboundPriority::System
    } else {
      OutboundPriority::User
    }
  }

  fn map_error(&self, error: EndpointWriterError, message: AnyMessage) -> SendError {
    match error {
      | EndpointWriterError::QueueFull(_) => SendError::full(message),
      | EndpointWriterError::QueueClosed(_) | EndpointWriterError::QueueUnavailable { .. } => {
        SendError::closed(message)
      },
      | EndpointWriterError::Serialization(_) => SendError::closed(message),
    }
  }

  fn enrich_sender_path(&self, sender_path: &ActorPath) -> ActorPath {
    if sender_path.parts().authority_endpoint().is_some() {
      return sender_path.clone();
    }

    let mut parts = sender_path.parts().clone();
    let authority_components = self.writer.with_read(|w| w.canonical_authority_components());
    if let Some((host, port)) = authority_components {
      parts = parts.with_scheme(ActorPathScheme::FraktorTcp);
      parts = parts.with_authority_host(host);
      if let Some(port) = port {
        parts = parts.with_authority_port(port);
      }
    }

    let mut rebuilt = ActorPath::from_parts(parts.clone());
    let guardian = parts.guardian_segment();
    let segments = sender_path.segments();
    let start = segments.first().is_some_and(|segment| segment.as_str() == guardian) as usize;
    for segment in segments.iter().skip(start) {
      rebuilt = rebuilt.child(segment.as_str());
    }
    if let Some(uid) = sender_path.uid() {
      rebuilt = rebuilt.with_uid(uid);
    }
    rebuilt
  }
}

impl ActorRefSender for RemoteActorRefSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let system_state =
      self.writer.with_read(|w| w.system().map(|s| s.state())).ok_or_else(|| SendError::closed(message.clone()))?;
    let normalizer = ActorRefFieldNormalizerGeneric::new(system_state);
    if let Err(RemoteAuthorityError::Quarantined) = normalizer.validate_recipient(&self.recipient) {
      return Err(SendError::closed(message));
    }
    if let Err(RemoteAuthorityError::Quarantined) = normalizer.validate_sender(&message) {
      return Err(SendError::closed(message));
    }

    let priority = Self::determine_priority(&message);
    let message_clone = message.clone();
    let mut outbound = match priority {
      | OutboundPriority::System => {
        OutboundMessage::system(message.clone(), self.recipient.clone(), self.remote_node.clone())
      },
      | OutboundPriority::User => {
        OutboundMessage::user(message.clone(), self.recipient.clone(), self.remote_node.clone())
      },
    };
    if let Some(sender) = message.sender()
      && let Some(sender_path) = sender.path()
    {
      let enriched = self.enrich_sender_path(&sender_path);
      outbound = outbound.with_sender(enriched);
    }
    match loopback_router::try_deliver(&self.remote_node, &self.writer, outbound) {
      | Ok(LoopbackDeliveryOutcome::Delivered) => Ok(SendOutcome::Delivered),
      | Ok(LoopbackDeliveryOutcome::Pending(pending)) => self
        .writer
        .with_write(|w| w.enqueue(*pending))
        .map(|()| SendOutcome::Delivered)
        .map_err(|error| self.map_error(error, message_clone)),
      | Err(error) => Err(self.map_error(error, message_clone)),
    }
  }
}

struct RemoteWatchEntry {
  path:     ActorPath,
  watchers: Vec<Pid>,
}

impl RemoteWatchEntry {
  fn new(path: ActorPath) -> Self {
    Self { path, watchers: Vec::new() }
  }

  fn add_watcher(&mut self, watcher: Pid) -> bool {
    if self.watchers.contains(&watcher) {
      false
    } else {
      self.watchers.push(watcher);
      true
    }
  }

  fn remove_watcher(&mut self, watcher: Pid) -> bool {
    if let Some(index) = self.watchers.iter().position(|existing| *existing == watcher) {
      self.watchers.swap_remove(index);
      true
    } else {
      false
    }
  }

  fn target_parts(&self) -> ActorPathParts {
    self.path.parts().clone()
  }

  #[cfg(any(test, feature = "test-support"))]
  fn watchers(&self) -> &[Pid] {
    &self.watchers
  }
}

impl ActorRefProvider for RemoteActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::FraktorTcp]
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    Self::actor_ref(self, path).map_err(|error| ActorError::fatal(format!("{error}")))
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    self.state()?.root_guardian().map(|cell| cell.actor_ref())
  }

  fn guardian(&self) -> Option<ActorRef> {
    self.state()?.user_guardian().map(|cell| cell.actor_ref())
  }

  fn system_guardian(&self) -> Option<ActorRef> {
    self.state()?.system_guardian().map(|cell| cell.actor_ref())
  }

  fn root_path(&self) -> ActorPath {
    self.state().map_or_else(
      || ActorPath::from_parts(ActorPathParts::local("cellactor")),
      |state| Self::root_path_for_state(&state),
    )
  }

  fn root_guardian_at(&self, address: &Address) -> Option<ActorRef> {
    let default = self.get_default_address()?;
    if address.protocol() == default.protocol() { self.root_guardian() } else { None }
  }

  fn deployer(&self) -> Option<Deployer> {
    Some(self.state()?.deployer())
  }

  fn temp_path(&self) -> ActorPath {
    self.root_path().child("temp")
  }

  fn temp_path_with_prefix(&self, prefix: &str) -> Result<ActorPath, ActorError> {
    let state = self.state().ok_or_else(|| ActorError::fatal("remote provider system unavailable"))?;
    let generated = if prefix.is_empty() {
      state.next_temp_actor_name_with_prefix("tmp")
    } else {
      state.next_temp_actor_name_with_prefix(prefix)
    };
    self.temp_path().try_child(&generated).map_err(|error| ActorError::fatal(format!("invalid temp path: {error}")))
  }

  fn temp_container(&self) -> Option<ActorRef> {
    let state = self.state()?;
    state.register_actor_path(PROVIDER_TEMP_CONTAINER_PID, &self.temp_path());
    Some(ActorRef::with_system(PROVIDER_TEMP_CONTAINER_PID, NullSender, &state))
  }

  fn register_temp_actor(&self, actor: ActorRef) -> Option<String> {
    Some(self.state()?.register_temp_actor(actor))
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

  fn termination_future(&self) -> ActorFutureShared<()> {
    self.state().map_or_else(ActorFutureShared::new, |state| state.termination_future())
  }

  fn get_external_address_for(&self, addr: &Address) -> Option<Address> {
    let default = self.get_default_address()?;
    if addr.protocol() == default.protocol() { Some(default) } else { None }
  }

  fn get_default_address(&self) -> Option<Address> {
    Self::default_address_from_state(&self.state()?)
  }
}
