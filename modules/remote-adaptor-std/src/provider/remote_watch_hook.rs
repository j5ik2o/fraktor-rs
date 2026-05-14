//! Remote watch hook installed into actor-core.

use std::time::Instant;

use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::ActorPath,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  event::stream::CorrelationId,
  system::{remote::RemoteWatchHook, state::SystemStateShared},
};
use fraktor_remote_core_rs::{
  address::RemoteNodeId,
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::RemoteEvent,
  provider::resolve_remote_address,
  transport::TransportEndpoint,
  watcher::WatcherCommand,
};
use fraktor_utils_core_rs::sync::{SharedAccess, SharedLock};
use tokio::sync::mpsc::{Sender, error::TrySendError};

use super::remote_actor_path_registry::RemoteActorPathRegistry;
use crate::association::std_instant_elapsed_millis;

pub(crate) struct StdRemoteWatchHook {
  registry:        SharedLock<RemoteActorPathRegistry>,
  state:           SystemStateShared,
  event_sender:    Sender<RemoteEvent>,
  watcher_sender:  Sender<WatcherCommand>,
  monotonic_epoch: Instant,
}

impl StdRemoteWatchHook {
  pub(crate) const fn new(
    registry: SharedLock<RemoteActorPathRegistry>,
    state: SystemStateShared,
    event_sender: Sender<RemoteEvent>,
    watcher_sender: Sender<WatcherCommand>,
    monotonic_epoch: Instant,
  ) -> Self {
    Self { registry, state, event_sender, watcher_sender, monotonic_epoch }
  }

  fn remote_path_for(&self, pid: &Pid) -> Option<ActorPath> {
    self.registry.with_read(|registry| registry.path_for_pid(pid))
  }

  fn send_watcher_command(&self, command: WatcherCommand) -> bool {
    match self.watcher_sender.try_send(command) {
      | Ok(()) => true,
      | Err(TrySendError::Full(command) | TrySendError::Closed(command)) => {
        tracing::warn!(?command, "remote watch command queue rejected command");
        true
      },
    }
  }

  fn enqueue_system_message(&self, recipient: ActorPath, sender: Option<ActorPath>, message: SystemMessage) -> bool {
    let Some(unique_address) = resolve_remote_address(&recipient) else {
      return false;
    };
    let address = unique_address.address().clone();
    let remote_node = RemoteNodeId::new(
      address.system().to_string(),
      address.host().to_string(),
      Some(address.port()),
      unique_address.uid(),
    );
    let event = RemoteEvent::OutboundEnqueued {
      authority: TransportEndpoint::new(address.to_string()),
      envelope:  Box::new(OutboundEnvelope::new(
        recipient,
        sender,
        AnyMessage::new(message),
        OutboundPriority::System,
        remote_node,
        CorrelationId::nil(),
      )),
      now_ms:    std_instant_elapsed_millis(self.monotonic_epoch),
    };
    match self.event_sender.try_send(event) {
      | Ok(()) => true,
      | Err(TrySendError::Full(_) | TrySendError::Closed(_)) => {
        tracing::warn!("remote watch notification event queue rejected envelope");
        true
      },
    }
  }
}

impl RemoteWatchHook for StdRemoteWatchHook {
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    let Some(target) = self.remote_path_for(&target) else {
      return false;
    };
    let Some(watcher) = self.state.canonical_actor_path(&watcher) else {
      return false;
    };
    self.send_watcher_command(WatcherCommand::Watch { target, watcher })
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    let Some(target) = self.remote_path_for(&target) else {
      return false;
    };
    let Some(watcher) = self.state.canonical_actor_path(&watcher) else {
      return false;
    };
    self.send_watcher_command(WatcherCommand::Unwatch { target, watcher })
  }

  fn handle_deathwatch_notification(&mut self, watcher: Pid, terminated: Pid) -> bool {
    let Some(recipient) = self.remote_path_for(&watcher) else {
      return false;
    };
    let Some(sender) = self.state.canonical_actor_path(&terminated) else {
      tracing::warn!(%watcher, %terminated, "remote death-watch notification target path is unavailable");
      return true;
    };
    self.enqueue_system_message(recipient, Some(sender), SystemMessage::DeathWatchNotification(terminated))
  }
}
