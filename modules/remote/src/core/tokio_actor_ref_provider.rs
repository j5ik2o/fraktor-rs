//! Provides actor references targeting remote authorities using Tokio TCP transport.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

use ahash::RandomState;
use fraktor_actor_rs::core::{
  actor_prim::{
    Pid,
    actor_path::{ActorPath, ActorPathParts},
    actor_ref::{ActorRefGeneric, ActorRefSender},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, SystemMessage},
  system::{
    ActorRefProvider, ActorSystemGeneric, RemoteAuthorityError, RemoteAuthorityManagerGeneric, RemoteWatchHook,
  },
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};
use hashbrown::HashMap;

use crate::core::{
  EndpointWriterGeneric, actor_ref_field_normalizer::ActorRefFieldNormalizerGeneric,
  endpoint_writer_error::EndpointWriterError, loopback_router, loopback_router::LoopbackDeliveryOutcome,
  outbound_message::OutboundMessage, outbound_priority::OutboundPriority,
  remote_actor_ref_provider_error::RemoteActorRefProviderError, remote_authority_snapshot::RemoteAuthoritySnapshot,
  remote_node_id::RemoteNodeId, remote_watcher_command::RemoteWatcherCommand,
  remote_watcher_daemon::RemoteWatcherDaemon, remoting_control::RemotingControl,
  remoting_control_handle::RemotingControlHandle, remoting_error::RemotingError, transport::TokioTransportConfig,
};

/// Provider that creates [`ActorRefGeneric`] instances for remote recipients using Tokio TCP
/// transport.
pub struct TokioActorRefProviderGeneric<TB: RuntimeToolbox + 'static> {
  system:            ActorSystemGeneric<TB>,
  writer:            ArcShared<EndpointWriterGeneric<TB>>,
  control:           RemotingControlHandle<TB>,
  authority_manager: ArcShared<RemoteAuthorityManagerGeneric<TB>>,
  watcher_daemon:    ActorRefGeneric<TB>,
  watch_entries:     NoStdMutex<HashMap<Pid, RemoteWatchEntry, RandomState>>,
  #[allow(dead_code)] // Reserved for future transport-specific configuration
  transport_config: TokioTransportConfig,
}

/// Provider that creates [`ActorRefGeneric`] instances for remote recipients using Tokio TCP
/// transport.
pub type TokioActorRefProvider = TokioActorRefProviderGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> TokioActorRefProviderGeneric<TB> {
  /// Creates a remote actor reference for the provided path.
  pub fn actor_ref(&self, path: ActorPath) -> Result<ActorRefGeneric<TB>, RemoteActorRefProviderError> {
    self.control.associate(path.parts()).map_err(RemoteActorRefProviderError::from)?;
    let sender = self.sender_for_path(&path)?;
    let pid = self.system.allocate_pid();
    self.register_remote_entry(pid, path.clone());
    Ok(ActorRefGeneric::with_system(pid, ArcShared::new(sender), self.system.state()))
  }

  pub(crate) fn from_components(
    system: ActorSystemGeneric<TB>,
    writer: ArcShared<EndpointWriterGeneric<TB>>,
    control: RemotingControlHandle<TB>,
    authority_manager: ArcShared<RemoteAuthorityManagerGeneric<TB>>,
    transport_config: TokioTransportConfig,
  ) -> Result<Self, RemoteActorRefProviderError> {
    let daemon = RemoteWatcherDaemon::spawn(&system, control.clone())?;
    Ok(Self {
      system,
      writer,
      control,
      authority_manager,
      watcher_daemon: daemon,
      watch_entries: NoStdMutex::new(HashMap::with_hasher(RandomState::new())),
      transport_config,
    })
  }

  fn sender_for_path(&self, path: &ActorPath) -> Result<RemoteActorRefSender<TB>, RemoteActorRefProviderError> {
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
  pub fn writer_for_test(&self) -> ArcShared<EndpointWriterGeneric<TB>> {
    self.writer.clone()
  }

  /// Requests an association/watch with the provided remote address.
  pub fn watch_remote(&self, parts: ActorPathParts) -> Result<(), RemotingError> {
    let Some(authority) = parts.authority_endpoint() else {
      return Err(RemotingError::TransportUnavailable("missing authority".into()));
    };
    let _ = self.authority_manager.state(&authority);
    self.record_snapshot_from_parts(&parts);
    self.control.associate(&parts)
  }

  /// Returns the latest remote authority snapshots recorded by the control plane.
  #[must_use]
  pub fn connections_snapshot(&self) -> Vec<crate::core::remote_authority_snapshot::RemoteAuthoritySnapshot> {
    self.control.connections_snapshot()
  }

  fn register_remote_entry(&self, pid: Pid, path: ActorPath) {
    let mut guard = self.watch_entries.lock();
    guard.entry(pid).or_insert_with(|| RemoteWatchEntry::new(path.clone()));
    self.record_snapshot_from_parts(path.parts());
  }

  fn record_snapshot_from_parts(&self, parts: &ActorPathParts) {
    let Some(authority) = parts.authority_endpoint() else {
      return;
    };
    let deferred = self.authority_manager.deferred_count(&authority) as u32;
    let state = self.system.state().remote_authority_state(&authority);
    let ticks = self.system.state().monotonic_now().as_millis() as u64;
    let snapshot = RemoteAuthoritySnapshot::new(authority, state, ticks, deferred);
    self.control.record_authority_snapshot(snapshot);
  }

  fn dispatch_remote_watch(&self, command: RemoteWatcherCommand) {
    let _ = self.watcher_daemon.tell(AnyMessageGeneric::new(command));
  }

  fn track_watch(&self, target: Pid, watcher: Pid) -> Option<(ActorPathParts, bool)> {
    let mut guard = self.watch_entries.lock();
    guard.get_mut(&target).map(|entry| {
      let added = entry.add_watcher(watcher);
      (entry.target_parts(), added)
    })
  }

  fn track_unwatch(&self, target: Pid, watcher: Pid) -> Option<(ActorPathParts, bool)> {
    let mut guard = self.watch_entries.lock();
    guard.get_mut(&target).map(|entry| {
      let removed = entry.remove_watcher(watcher);
      (entry.target_parts(), removed)
    })
  }

  #[cfg(any(test, feature = "test-support"))]
  /// Returns the set of remote PIDs tracked by the provider (test helper).
  pub fn registered_remote_pids_for_test(&self) -> Vec<Pid> {
    self.watch_entries.lock().keys().copied().collect()
  }

  #[cfg(any(test, feature = "test-support"))]
  /// Returns the watchers registered for a remote PID (test helper).
  pub fn remote_watchers_for_test(&self, pid: Pid) -> Option<Vec<Pid>> {
    self.watch_entries.lock().get(&pid).map(|entry| entry.watchers().to_vec())
  }
}

impl<TB: RuntimeToolbox + 'static> RemoteWatchHook<TB> for TokioActorRefProviderGeneric<TB> {
  fn handle_watch(&self, target: Pid, watcher: Pid) -> bool {
    if let Some((parts, should_send)) = self.track_watch(target, watcher) {
      if should_send {
        self.dispatch_remote_watch(RemoteWatcherCommand::Watch { target: parts, watcher });
      }
      true
    } else {
      false
    }
  }

  fn handle_unwatch(&self, target: Pid, watcher: Pid) -> bool {
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

struct RemoteActorRefSender<TB: RuntimeToolbox + 'static> {
  writer:      ArcShared<EndpointWriterGeneric<TB>>,
  recipient:   ActorPath,
  remote_node: RemoteNodeId,
}

impl<TB: RuntimeToolbox + 'static> RemoteActorRefSender<TB> {
  fn determine_priority(message: &AnyMessageGeneric<TB>) -> OutboundPriority {
    if message.as_view().downcast_ref::<SystemMessage>().is_some() {
      OutboundPriority::System
    } else {
      OutboundPriority::User
    }
  }

  fn map_error(&self, error: EndpointWriterError, message: AnyMessageGeneric<TB>) -> SendError<TB> {
    match error {
      | EndpointWriterError::QueueFull(_) => SendError::full(message),
      | EndpointWriterError::QueueClosed(_) | EndpointWriterError::QueueUnavailable { .. } => {
        SendError::closed(message)
      },
      | EndpointWriterError::Serialization(_) => SendError::closed(message),
    }
  }

  fn enrich_reply_path(&self, reply_path: &ActorPath) -> ActorPath {
    if reply_path.parts().authority_endpoint().is_some() {
      return reply_path.clone();
    }

    let mut parts = reply_path.parts().clone();
    if let Some((host, port)) = self.writer.canonical_authority_components() {
      parts = parts.with_authority_host(host);
      if let Some(port) = port {
        parts = parts.with_authority_port(port);
      }
    }

    let mut rebuilt = ActorPath::from_parts(parts);
    for segment in reply_path.segments() {
      rebuilt = rebuilt.child(segment.as_str());
    }
    if let Some(uid) = reply_path.uid() {
      rebuilt = rebuilt.with_uid(uid);
    }
    rebuilt
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for RemoteActorRefSender<TB> {
  fn send(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    let normalizer = ActorRefFieldNormalizerGeneric::new(self.writer.system().state());
    if let Err(RemoteAuthorityError::Quarantined) = normalizer.validate_recipient(&self.recipient) {
      return Err(SendError::closed(message));
    }
    if let Err(RemoteAuthorityError::Quarantined) = normalizer.validate_reply_to(&message) {
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
    if let Some(reply_to) = message.reply_to()
      && let Some(reply_path) = reply_to.path()
    {
      let enriched = self.enrich_reply_path(&reply_path);
      outbound = outbound.with_reply_to(enriched);
    }
    match loopback_router::try_deliver(&self.remote_node, &self.writer, outbound) {
      | Ok(LoopbackDeliveryOutcome::Delivered) => Ok(()),
      | Ok(LoopbackDeliveryOutcome::Pending(pending)) => {
        self.writer.enqueue(*pending).map_err(|error| self.map_error(error, message_clone))
      },
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

/// Implementation of ActorRefProvider trait for Tokio TCP transport.
impl<TB: RuntimeToolbox + 'static> ActorRefProvider<TB> for TokioActorRefProviderGeneric<TB> {
  fn supported_schemes(&self) -> &'static [fraktor_actor_rs::core::actor_prim::actor_path::ActorPathScheme] {
    &[fraktor_actor_rs::core::actor_prim::actor_path::ActorPathScheme::FraktorTcp]
  }

  fn actor_ref(&self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    Self::actor_ref(self, path)
      .map_err(|error| ActorError::fatal(alloc::format!("Failed to create Tokio actor ref: {:?}", error)))
  }
}
