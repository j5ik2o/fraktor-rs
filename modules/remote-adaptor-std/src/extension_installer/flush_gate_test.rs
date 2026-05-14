use std::{string::String, time::Duration};

use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::ActorPathParser,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  event::stream::CorrelationId,
};
use fraktor_remote_core_rs::{
  address::RemoteNodeId,
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::{RemoteEvent, RemoteFlushOutcome},
  transport::TransportEndpoint,
  wire::FlushScope,
};
use tokio::sync::mpsc::{self, Receiver};

use super::*;

fn test_authority() -> TransportEndpoint {
  TransportEndpoint::new("remote-sys@10.0.0.1:2552")
}

fn test_envelope() -> OutboundEnvelope {
  let recipient =
    ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/watcher").expect("recipient path");
  let sender = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/terminated").expect("sender path");
  OutboundEnvelope::new(
    recipient,
    Some(sender),
    AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(10, 0))),
    OutboundPriority::System,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  )
}

fn push_pending_notification(gate: &StdFlushGate, authority: TransportEndpoint, flush_ids: Vec<u64>) {
  gate.inner.lock().expect(FLUSH_GATE_LOCK_POISONED).pending_notifications.push(PendingNotification {
    authority,
    flush_ids,
    envelope: test_envelope(),
    now_ms: 42,
  });
}

fn assert_one_notification_enqueued(receiver: &mut Receiver<RemoteEvent>) {
  assert!(matches!(
    receiver.try_recv(),
    Ok(RemoteEvent::OutboundEnqueued { envelope, now_ms: 42, .. })
      if envelope.priority() == OutboundPriority::System
        && envelope.message().downcast_ref::<SystemMessage>()
          == Some(&SystemMessage::DeathWatchNotification(Pid::new(10, 0)))
  ));
  assert!(receiver.try_recv().is_err());
}

#[test]
fn completed_flush_releases_pending_notification_once() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, mut event_rx) = mpsc::channel(4);
  push_pending_notification(&gate, authority.clone(), vec![1]);
  let outcome = RemoteFlushOutcome::Completed {
    authority: authority.clone(),
    flush_id:  1,
    scope:     FlushScope::BeforeDeathWatchNotification,
  };

  gate.observe_outcomes(vec![outcome.clone()], &event_tx);
  gate.observe_outcomes(vec![outcome], &event_tx);

  assert_one_notification_enqueued(&mut event_rx);
}

#[test]
fn timed_out_flush_releases_pending_notification() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, mut event_rx) = mpsc::channel(4);
  push_pending_notification(&gate, authority.clone(), vec![1]);

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::TimedOut {
      authority,
      flush_id: 1,
      scope: FlushScope::BeforeDeathWatchNotification,
      pending_lanes: vec![0],
    }],
    &event_tx,
  );

  assert_one_notification_enqueued(&mut event_rx);
}

#[test]
fn failed_flush_releases_pending_notification() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, mut event_rx) = mpsc::channel(4);
  push_pending_notification(&gate, authority.clone(), vec![1]);

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Failed {
      authority,
      flush_id: 1,
      scope: FlushScope::BeforeDeathWatchNotification,
      pending_lanes: vec![0],
      reason: String::from("send failed"),
    }],
    &event_tx,
  );

  assert_one_notification_enqueued(&mut event_rx);
}

#[test]
fn pending_notification_waits_for_all_flush_ids() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, mut event_rx) = mpsc::channel(4);
  push_pending_notification(&gate, authority.clone(), vec![1, 2]);

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Completed {
      authority: authority.clone(),
      flush_id:  1,
      scope:     FlushScope::BeforeDeathWatchNotification,
    }],
    &event_tx,
  );
  assert!(event_rx.try_recv().is_err());
  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Completed { authority, flush_id: 2, scope: FlushScope::BeforeDeathWatchNotification }],
    &event_tx,
  );

  assert_one_notification_enqueued(&mut event_rx);
}

#[test]
fn pending_notification_ignores_outcome_for_other_authority() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, mut event_rx) = mpsc::channel(4);
  push_pending_notification(&gate, authority.clone(), vec![1]);

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Completed {
      authority: TransportEndpoint::new("other-sys@10.0.0.2:2552"),
      flush_id:  1,
      scope:     FlushScope::BeforeDeathWatchNotification,
    }],
    &event_tx,
  );

  assert!(event_rx.try_recv().is_err());
  let pending = &gate.inner.lock().expect(FLUSH_GATE_LOCK_POISONED).pending_notifications;
  assert_eq!(pending.len(), 1);
  assert_eq!(pending[0].authority, authority);
  assert_eq!(pending[0].flush_ids, vec![1]);
}

#[test]
fn pending_notification_release_observes_full_event_queue() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, mut event_rx) = mpsc::channel(1);
  event_tx.try_send(RemoteEvent::TransportShutdown).expect("event queue should accept first event");
  push_pending_notification(&gate, authority.clone(), vec![1]);

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Completed {
      authority: authority.clone(),
      flush_id:  1,
      scope:     FlushScope::BeforeDeathWatchNotification,
    }],
    &event_tx,
  );

  assert!(matches!(event_rx.try_recv(), Ok(RemoteEvent::TransportShutdown)));
  assert_eq!(gate.inner.lock().expect(FLUSH_GATE_LOCK_POISONED).pending_notifications.len(), 1);

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Completed { authority, flush_id: 1, scope: FlushScope::BeforeDeathWatchNotification }],
    &event_tx,
  );
  assert_one_notification_enqueued(&mut event_rx);
}

#[test]
fn pending_notification_release_observes_closed_event_queue() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, event_rx) = mpsc::channel(1);
  drop(event_rx);
  push_pending_notification(&gate, authority.clone(), vec![1]);

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Completed { authority, flush_id: 1, scope: FlushScope::BeforeDeathWatchNotification }],
    &event_tx,
  );
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn immediate_notification_full_event_queue_defers_delivery() {
  let authority = test_authority();
  let (event_tx, mut event_rx) = mpsc::channel(1);
  event_tx.try_send(RemoteEvent::TransportShutdown).expect("event queue should accept first event");

  assert!(enqueue_outbound(&event_tx, authority.clone(), test_envelope(), 42));
  assert!(matches!(event_rx.try_recv(), Ok(RemoteEvent::TransportShutdown)));
  let event = tokio::time::timeout(Duration::from_millis(50), event_rx.recv())
    .await
    .expect("deferred notification should be delivered")
    .expect("event queue should remain open");
  assert!(matches!(
    event,
    RemoteEvent::OutboundEnqueued { authority: received_authority, envelope, now_ms: 42 }
      if received_authority == authority
        && envelope.priority() == OutboundPriority::System
        && envelope.message().downcast_ref::<SystemMessage>()
          == Some(&SystemMessage::DeathWatchNotification(Pid::new(10, 0)))
  ));
  assert!(event_rx.try_recv().is_err());
}

#[test]
fn empty_shutdown_waiter_is_not_registered() {
  let gate = StdFlushGate::new();

  assert!(gate.register_shutdown_waiter(&[]).is_none());
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn shutdown_waiter_completes_after_matching_flush_outcome() {
  let gate = StdFlushGate::new();
  let authority = test_authority();
  let (event_tx, _event_rx) = mpsc::channel(4);
  let waiter = StdFlushWaitHandle::new(vec![FlushKey { authority: authority.clone(), flush_id: 1 }]);
  gate.inner.lock().expect(FLUSH_GATE_LOCK_POISONED).shutdown_waiters.push(waiter.clone());

  gate.observe_outcomes(
    vec![RemoteFlushOutcome::Completed { authority, flush_id: 1, scope: FlushScope::Shutdown }],
    &event_tx,
  );

  assert!(waiter.wait(Duration::from_millis(50)).await);
}
