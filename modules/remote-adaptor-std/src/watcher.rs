//! Tokio task that applies remote watcher effects.

#[cfg(test)]
#[path = "watcher_test.rs"]
mod tests;

use std::time::{Duration, Instant};

use fraktor_actor_core_kernel_rs::{
  actor::{Pid, actor_path::ActorPath, messaging::system_message::SystemMessage},
  event::stream::CorrelationId,
  system::ActorSystem,
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId},
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::RemoteEvent,
  failure_detector::PhiAccrualFailureDetector,
  provider::resolve_remote_address,
  transport::TransportEndpoint,
  watcher::{WatcherCommand, WatcherEffect, WatcherState},
  wire::ControlPdu,
};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::association::std_instant_elapsed_millis;

pub(crate) async fn run_watcher_task(
  mut commands: Receiver<WatcherCommand>,
  event_sender: Sender<RemoteEvent>,
  system: ActorSystem,
  local_address: Address,
  monotonic_epoch: Instant,
  tick_interval: Duration,
) {
  let mut state = WatcherState::new(default_detector_factory);
  let mut ticker = tokio::time::interval(tick_interval);
  loop {
    tokio::select! {
      command = commands.recv() => {
        let Some(command) = command else {
          return;
        };
        let now_ms = std_instant_elapsed_millis(monotonic_epoch);
        apply_effects(state.handle(command), &event_sender, &system, &local_address, monotonic_epoch, now_ms).await;
      },
      _ = ticker.tick() => {
        let now_ms = std_instant_elapsed_millis(monotonic_epoch);
        apply_effects(
          state.handle(WatcherCommand::HeartbeatTick { now: now_ms }),
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
  let event = RemoteEvent::OutboundControl {
    remote,
    pdu: ControlPdu::Heartbeat { authority: local_address.to_string() },
    now_ms,
  };
  if let Err(error) = event_sender.send(event).await {
    tracing::warn!(?error, "remote watcher heartbeat enqueue failed");
  }
}

async fn send_redelivery_tick(event_sender: &Sender<RemoteEvent>, remote: Address, now_ms: u64) {
  let event = RemoteEvent::RedeliveryTimerFired { authority: TransportEndpoint::new(remote.to_string()), now_ms };
  if let Err(error) = event_sender.send(event).await {
    tracing::warn!(?error, "remote watcher redelivery tick enqueue failed");
  }
}

async fn send_system_envelope(
  event_sender: &Sender<RemoteEvent>,
  recipient: ActorPath,
  sender: Option<ActorPath>,
  message: SystemMessage,
  monotonic_epoch: Instant,
) {
  let Some(unique_address) = resolve_remote_address(&recipient) else {
    tracing::warn!(recipient = %recipient, "remote watcher system envelope recipient is not remote");
    return;
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
      message.into(),
      OutboundPriority::System,
      remote_node,
      CorrelationId::nil(),
    )),
    now_ms:    std_instant_elapsed_millis(monotonic_epoch),
  };
  if let Err(error) = event_sender.send(event).await {
    tracing::warn!(?error, "remote watcher system envelope enqueue failed");
  }
}

fn default_detector_factory(address: &Address) -> PhiAccrualFailureDetector {
  PhiAccrualFailureDetector::new(address.clone(), 5.0, 100, 10, 0, 100)
}
