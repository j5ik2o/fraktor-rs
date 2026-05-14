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
  extension::{RemoteEvent, RemoteShared},
  provider::resolve_remote_address,
  transport::TransportEndpoint,
  watcher::WatcherCommand,
};
use fraktor_utils_core_rs::sync::{SharedAccess, SharedLock};
use tokio::sync::mpsc::{Sender, error::TrySendError};

use super::remote_actor_path_registry::RemoteActorPathRegistry;
use crate::{
  association::std_instant_elapsed_millis,
  extension_installer::{StdFlushGate, StdFlushNotification},
};

/// Flush dependencies used by remote-bound DeathWatch notification delivery.
pub(crate) struct StdRemoteWatchFlushConfig {
  remote_shared:  RemoteShared,
  flush_gate:     StdFlushGate,
  flush_lane_ids: Vec<u32>,
}

impl StdRemoteWatchFlushConfig {
  pub(crate) const fn new(remote_shared: RemoteShared, flush_gate: StdFlushGate, flush_lane_ids: Vec<u32>) -> Self {
    Self { remote_shared, flush_gate, flush_lane_ids }
  }
}

pub(crate) struct StdRemoteWatchHook {
  registry:        SharedLock<RemoteActorPathRegistry>,
  state:           SystemStateShared,
  event_sender:    Sender<RemoteEvent>,
  watcher_sender:  Sender<WatcherCommand>,
  monotonic_epoch: Instant,
  remote_shared:   RemoteShared,
  flush_gate:      StdFlushGate,
  flush_lane_ids:  Vec<u32>,
}

impl StdRemoteWatchHook {
  pub(crate) fn new_with_flush_gate(
    registry: SharedLock<RemoteActorPathRegistry>,
    state: SystemStateShared,
    event_sender: Sender<RemoteEvent>,
    watcher_sender: Sender<WatcherCommand>,
    monotonic_epoch: Instant,
    flush_config: StdRemoteWatchFlushConfig,
  ) -> Self {
    Self {
      registry,
      state,
      event_sender,
      watcher_sender,
      monotonic_epoch,
      remote_shared: flush_config.remote_shared,
      flush_gate: flush_config.flush_gate,
      flush_lane_ids: flush_config.flush_lane_ids,
    }
  }

  fn remote_path_for(&self, pid: &Pid) -> Option<ActorPath> {
    self.registry.with_read(|registry| registry.path_for_pid(pid))
  }

  fn send_watcher_command(&self, command: WatcherCommand) -> bool {
    match self.watcher_sender.try_send(command) {
      | Ok(()) => true,
      | Err(TrySendError::Full(command)) => {
        tracing::warn!(?command, "remote watch command queue is full");
        true
      },
      | Err(TrySendError::Closed(command)) => {
        tracing::warn!(?command, "remote watch command queue is closed");
        true
      },
    }
  }

  fn system_message_envelope(
    &self,
    recipient: ActorPath,
    sender: Option<ActorPath>,
    message: SystemMessage,
  ) -> Option<(TransportEndpoint, OutboundEnvelope, u64)> {
    let unique_address = resolve_remote_address(&recipient)?;
    let address = unique_address.address().clone();
    let remote_node = RemoteNodeId::new(
      address.system().to_string(),
      address.host().to_string(),
      Some(address.port()),
      unique_address.uid(),
    );
    let now_ms = std_instant_elapsed_millis(self.monotonic_epoch);
    Some((
      TransportEndpoint::new(address.to_string()),
      OutboundEnvelope::new(
        recipient,
        sender,
        AnyMessage::new(message),
        OutboundPriority::System,
        remote_node,
        CorrelationId::nil(),
      ),
      now_ms,
    ))
  }

  fn enqueue_system_message_after_flush(
    &self,
    recipient: ActorPath,
    sender: Option<ActorPath>,
    message: SystemMessage,
  ) -> bool {
    let Some((authority, envelope, now_ms)) = self.system_message_envelope(recipient, sender, message) else {
      return false;
    };
    self.flush_gate.submit_notification(&self.remote_shared, StdFlushNotification {
      event_sender: &self.event_sender,
      monotonic_epoch: self.monotonic_epoch,
      lane_ids: &self.flush_lane_ids,
      authority,
      envelope,
      now_ms,
    })
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
    let sender = self.state.canonical_actor_path(&terminated);
    if sender.is_none() {
      tracing::warn!(%watcher, %terminated, "remote death-watch notification target path is unavailable");
    }
    self.enqueue_system_message_after_flush(recipient, sender, SystemMessage::DeathWatchNotification(terminated))
  }
}
