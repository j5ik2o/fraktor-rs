//! Tokio task that applies remote watcher effects.

#[cfg(test)]
#[path = "watcher_test.rs"]
mod tests;

use std::time::{Duration, Instant};

use fraktor_actor_core_kernel_rs::{
  actor::{Pid, actor_path::ActorPath, messaging::system_message::SystemMessage},
  event::stream::{AddressTerminatedEvent, CorrelationId, EventStreamEvent},
  system::ActorSystem,
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId},
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::{RemoteEvent, RemoteShared},
  provider::resolve_remote_address,
  transport::TransportEndpoint,
  watcher::{WatcherCommand, WatcherEffect},
  wire::ControlPdu,
};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::association::std_instant_elapsed_millis;

pub(crate) async fn run_watcher_task(
  mut commands: Receiver<WatcherCommand>,
  remote: RemoteShared,
  event_sender: Sender<RemoteEvent>,
  system: ActorSystem,
  local_address: Address,
  monotonic_epoch: Instant,
  tick_interval: Duration,
) {
  let mut ticker = tokio::time::interval(tick_interval);
  loop {
    tokio::select! {
      command = commands.recv() => {
        let Some(command) = command else {
          return;
        };
        let now_ms = std_instant_elapsed_millis(monotonic_epoch);
        let effects = remote.handle_watcher_command_and_drain_effects(command);
        apply_effects(effects, &event_sender, &system, &local_address, monotonic_epoch, now_ms).await;
      },
      _ = ticker.tick() => {
        let now_ms = std_instant_elapsed_millis(monotonic_epoch);
        let effects = remote.handle_watcher_command_and_drain_effects(WatcherCommand::HeartbeatTick { now: now_ms });
        apply_effects(
          effects,
          &event_sender,
          &system,
          &local_address,
          monotonic_epoch,
          now_ms,
        ).await;
      },
    }
  }
}

pub(crate) async fn apply_effects(
  effects: Vec<WatcherEffect>,
  event_sender: &Sender<RemoteEvent>,
  system: &ActorSystem,
  local_address: &Address,
  monotonic_epoch: Instant,
  now_ms: u64,
) {
  for effect in effects {
    match effect {
      | WatcherEffect::SendWatch { target, watcher } => {
        send_system_envelope(
          event_sender,
          target,
          Some(watcher),
          SystemMessage::Watch(Pid::new(0, 0)),
          monotonic_epoch,
        )
        .await;
      },
      | WatcherEffect::SendUnwatch { target, watcher } => {
        send_system_envelope(
          event_sender,
          target,
          Some(watcher),
          SystemMessage::Unwatch(Pid::new(0, 0)),
          monotonic_epoch,
        )
        .await;
      },
      | WatcherEffect::SendHeartbeat { to } => {
        send_heartbeat(event_sender, local_address, to.clone(), now_ms).await;
        send_redelivery_tick(event_sender, to, now_ms).await;
      },
      | WatcherEffect::NotifyTerminated { target, watchers } => notify_local_watchers(system, target, watchers),
      | WatcherEffect::AddressTerminated { node, reason, observed_at_millis } => {
        publish_address_terminated(system, node, reason, observed_at_millis);
      },
      | WatcherEffect::NotifyQuarantined { node } => {
        tracing::warn!(remote = %node, "remote watcher marked node quarantined");
      },
      | WatcherEffect::RewatchRemoteTargets { watches, .. } => {
        for (target, watcher) in watches {
          send_system_envelope(
            event_sender,
            target,
            Some(watcher),
            SystemMessage::Watch(Pid::new(0, 0)),
            monotonic_epoch,
          )
          .await;
        }
      },
    }
  }
}

pub(crate) fn try_apply_effects(
  effects: Vec<WatcherEffect>,
  event_sender: &Sender<RemoteEvent>,
  system: &ActorSystem,
  local_address: &Address,
  monotonic_epoch: Instant,
  now_ms: u64,
) {
  for effect in effects {
    match effect {
      | WatcherEffect::SendWatch { target, watcher } => {
        try_send_system_envelope(
          event_sender,
          target,
          Some(watcher),
          SystemMessage::Watch(Pid::new(0, 0)),
          monotonic_epoch,
        );
      },
      | WatcherEffect::SendUnwatch { target, watcher } => {
        try_send_system_envelope(
          event_sender,
          target,
          Some(watcher),
          SystemMessage::Unwatch(Pid::new(0, 0)),
          monotonic_epoch,
        );
      },
      | WatcherEffect::SendHeartbeat { to } => {
        try_send_heartbeat(event_sender, local_address, to.clone(), now_ms);
        try_send_redelivery_tick(event_sender, to, now_ms);
      },
      | WatcherEffect::NotifyTerminated { target, watchers } => notify_local_watchers(system, target, watchers),
      | WatcherEffect::AddressTerminated { node, reason, observed_at_millis } => {
        publish_address_terminated(system, node, reason, observed_at_millis);
      },
      | WatcherEffect::NotifyQuarantined { node } => {
        tracing::warn!(remote = %node, "remote watcher marked node quarantined");
      },
      | WatcherEffect::RewatchRemoteTargets { watches, .. } => {
        for (target, watcher) in watches {
          try_send_system_envelope(
            event_sender,
            target,
            Some(watcher),
            SystemMessage::Watch(Pid::new(0, 0)),
            monotonic_epoch,
          );
        }
      },
    }
  }
}

fn publish_address_terminated(system: &ActorSystem, node: Address, reason: String, observed_at_millis: u64) {
  let event = AddressTerminatedEvent::new(node.to_string(), reason, observed_at_millis);
  system.event_stream().publish(&EventStreamEvent::AddressTerminated(event));
}

fn notify_local_watchers(system: &ActorSystem, target: ActorPath, watchers: Vec<ActorPath>) {
  let target_ref = match system.resolve_actor_ref(target.clone()) {
    | Ok(target_ref) => target_ref,
    | Err(error) => {
      tracing::warn!(?error, target = %target, "remote watcher target resolution failed");
      return;
    },
  };
  for watcher in watchers {
    let watcher_ref = match system.resolve_actor_ref(watcher.clone()) {
      | Ok(watcher_ref) => watcher_ref,
      | Err(error) => {
        tracing::warn!(?error, watcher = %watcher, "remote watcher recipient resolution failed");
        continue;
      },
    };
    if let Err(error) =
      system.state().send_system_message(watcher_ref.pid(), SystemMessage::DeathWatchNotification(target_ref.pid()))
    {
      tracing::warn!(?error, watcher = %watcher, target = %target, "remote watcher notification delivery failed");
    }
  }
}

async fn send_heartbeat(event_sender: &Sender<RemoteEvent>, local_address: &Address, remote: Address, now_ms: u64) {
  let event = heartbeat_event(local_address, remote, now_ms);
  if let Err(error) = event_sender.send(event).await {
    tracing::warn!(?error, "remote watcher heartbeat enqueue failed");
  }
}

fn try_send_heartbeat(event_sender: &Sender<RemoteEvent>, local_address: &Address, remote: Address, now_ms: u64) {
  let event = heartbeat_event(local_address, remote, now_ms);
  if let Err(error) = event_sender.try_send(event) {
    tracing::warn!(?error, "remote watcher heartbeat enqueue failed");
  }
}

fn heartbeat_event(local_address: &Address, remote: Address, now_ms: u64) -> RemoteEvent {
  RemoteEvent::OutboundControl { remote, pdu: ControlPdu::Heartbeat { authority: local_address.to_string() }, now_ms }
}

async fn send_redelivery_tick(event_sender: &Sender<RemoteEvent>, remote: Address, now_ms: u64) {
  let event = redelivery_tick_event(remote, now_ms);
  if let Err(error) = event_sender.send(event).await {
    tracing::warn!(?error, "remote watcher redelivery tick enqueue failed");
  }
}

fn try_send_redelivery_tick(event_sender: &Sender<RemoteEvent>, remote: Address, now_ms: u64) {
  let event = redelivery_tick_event(remote, now_ms);
  if let Err(error) = event_sender.try_send(event) {
    tracing::warn!(?error, "remote watcher redelivery tick enqueue failed");
  }
}

fn redelivery_tick_event(remote: Address, now_ms: u64) -> RemoteEvent {
  RemoteEvent::RedeliveryTimerFired { authority: TransportEndpoint::new(remote.to_string()), now_ms }
}

async fn send_system_envelope(
  event_sender: &Sender<RemoteEvent>,
  recipient: ActorPath,
  sender: Option<ActorPath>,
  message: SystemMessage,
  monotonic_epoch: Instant,
) {
  let Some(event) = system_envelope_event(recipient, sender, message, monotonic_epoch) else {
    return;
  };
  if let Err(error) = event_sender.send(event).await {
    tracing::warn!(?error, "remote watcher system envelope enqueue failed");
  }
}

fn try_send_system_envelope(
  event_sender: &Sender<RemoteEvent>,
  recipient: ActorPath,
  sender: Option<ActorPath>,
  message: SystemMessage,
  monotonic_epoch: Instant,
) {
  let Some(event) = system_envelope_event(recipient, sender, message, monotonic_epoch) else {
    return;
  };
  if let Err(error) = event_sender.try_send(event) {
    tracing::warn!(?error, "remote watcher system envelope enqueue failed");
  }
}

fn system_envelope_event(
  recipient: ActorPath,
  sender: Option<ActorPath>,
  message: SystemMessage,
  monotonic_epoch: Instant,
) -> Option<RemoteEvent> {
  let Some(unique_address) = resolve_remote_address(&recipient) else {
    tracing::warn!(recipient = %recipient, "remote watcher system envelope recipient is not remote");
    return None;
  };
  let address = unique_address.address().clone();
  let remote_node = RemoteNodeId::new(
    address.system().to_string(),
    address.host().to_string(),
    Some(address.port()),
    unique_address.uid(),
  );
  Some(RemoteEvent::OutboundEnqueued {
    authority: TransportEndpoint::new(address.to_string()),
    envelope:  Box::new(OutboundEnvelope::new(
      recipient,
      sender,
      message.into(),
      OutboundPriority::System,
      remote_node,
      CorrelationId::nil(),
    )),
    now_ms:    std_instant_elapsed_millis(monotonic_epoch),
  })
}
