//! Shared std-side coordination for remote flush outcomes.

#[cfg(test)]
#[path = "flush_gate_test.rs"]
mod tests;

use std::{
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use fraktor_remote_core_rs::{
  envelope::OutboundEnvelope,
  extension::{RemoteEvent, RemoteFlushOutcome, RemoteFlushTimer, RemoteShared},
  transport::TransportEndpoint,
  wire::FlushScope,
};
use tokio::{
  sync::{
    Notify,
    mpsc::{self, Sender, error::TrySendError},
  },
  time::sleep,
};

use crate::association::std_instant_elapsed_millis;

const FLUSH_GATE_LOCK_POISONED: &str = "std flush gate lock should not be poisoned";
const RETRY_QUEUE_CAPACITY: usize = 64;

/// Coordinates std-owned waiters and pending notifications for flush outcomes.
#[derive(Clone)]
pub(crate) struct StdFlushGate {
  inner: Arc<Mutex<StdFlushGateState>>,
}

/// Input required to submit one pending remote-bound notification.
pub(crate) struct StdFlushNotification<'a> {
  /// Event sender used for timer and outbound events.
  pub(crate) event_sender:    &'a Sender<RemoteEvent>,
  /// Monotonic epoch used to compute timer delays.
  pub(crate) monotonic_epoch: Instant,
  /// Writer lane ids that must be flushed.
  pub(crate) lane_ids:        &'a [u32],
  /// Remote authority that should receive the notification.
  pub(crate) authority:       TransportEndpoint,
  /// Notification envelope to release after the flush outcome.
  pub(crate) envelope:        OutboundEnvelope,
  /// Monotonic millis associated with the outbound event.
  pub(crate) now_ms:          u64,
}

struct StdFlushGateState {
  pending_notifications: Vec<PendingNotification>,
  shutdown_waiters:      Vec<StdFlushWaitHandle>,
  retry_sender:          Option<Sender<RemoteEvent>>,
}

struct PendingNotification {
  authority: TransportEndpoint,
  flush_ids: Vec<u64>,
  envelope:  OutboundEnvelope,
  now_ms:    u64,
}

#[derive(Clone)]
pub(super) struct StdFlushWaitHandle {
  inner:  Arc<Mutex<StdFlushWaitState>>,
  notify: Arc<Notify>,
}

struct StdFlushWaitState {
  pending: Vec<FlushKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FlushKey {
  authority: TransportEndpoint,
  flush_id:  u64,
}

impl StdFlushGate {
  /// Creates an empty flush gate.
  #[must_use]
  pub(super) fn new() -> Self {
    Self {
      inner: Arc::new(Mutex::new(StdFlushGateState {
        pending_notifications: Vec::new(),
        shutdown_waiters:      Vec::new(),
        retry_sender:          None,
      })),
    }
  }

  /// Submits a remote-bound DeathWatch notification after starting a flush.
  pub(crate) fn submit_notification(
    &self,
    remote_shared: &RemoteShared,
    notification: StdFlushNotification<'_>,
  ) -> bool {
    let authority = notification.authority;
    let (timers, outcomes) = match remote_shared.start_flush_and_drain_outcomes(
      Some(&authority),
      FlushScope::BeforeDeathWatchNotification,
      notification.lane_ids,
      notification.now_ms,
    ) {
      | Ok(timers) => timers,
      | Err(error) => {
        tracing::warn!(?error, remote = %authority.authority(), "death-watch flush start failed");
        return self.enqueue_outbound(notification.event_sender, authority, notification.envelope, notification.now_ms);
      },
    };
    let mut flush_ids: Vec<u64> = timers.iter().map(RemoteFlushTimer::flush_id).collect();
    if flush_ids.is_empty() {
      flush_ids.extend(outcomes.iter().filter_map(|outcome| {
        (outcome.scope() == FlushScope::BeforeDeathWatchNotification && outcome.authority() == &authority)
          .then_some(outcome.flush_id())
      }));
    }
    if flush_ids.is_empty() {
      return self.enqueue_outbound(notification.event_sender, authority, notification.envelope, notification.now_ms);
    }
    {
      let mut state = self.inner.lock().expect(FLUSH_GATE_LOCK_POISONED);
      state.pending_notifications.push(PendingNotification {
        authority: authority.clone(),
        flush_ids,
        envelope: notification.envelope,
        now_ms: notification.now_ms,
      });
    }
    schedule_flush_timers(notification.event_sender, notification.monotonic_epoch, &timers);
    self.observe_outcomes(outcomes, notification.event_sender);
    true
  }

  /// Registers a shutdown waiter for the given timers.
  pub(super) fn register_shutdown_waiter(&self, timers: &[RemoteFlushTimer]) -> Option<StdFlushWaitHandle> {
    if timers.is_empty() {
      return None;
    }
    let handle = StdFlushWaitHandle::new(
      timers
        .iter()
        .map(|timer| FlushKey { authority: timer.authority().clone(), flush_id: timer.flush_id() })
        .collect(),
    );
    let mut state = self.inner.lock().expect(FLUSH_GATE_LOCK_POISONED);
    state.shutdown_waiters.push(handle.clone());
    Some(handle)
  }

  /// Applies flush outcomes to pending notifications and shutdown waiters.
  pub(super) fn observe_outcomes(&self, outcomes: Vec<RemoteFlushOutcome>, event_sender: &Sender<RemoteEvent>) {
    for outcome in outcomes {
      self.observe_outcome(outcome, event_sender);
    }
  }

  fn observe_outcome(&self, outcome: RemoteFlushOutcome, event_sender: &Sender<RemoteEvent>) {
    let key = FlushKey { authority: outcome.authority().clone(), flush_id: outcome.flush_id() };
    if matches!(outcome, RemoteFlushOutcome::TimedOut { .. } | RemoteFlushOutcome::Failed { .. }) {
      tracing::warn!(?outcome, "remote flush completed without ordering guarantee");
    }
    let ready = {
      let mut state = self.inner.lock().expect(FLUSH_GATE_LOCK_POISONED);
      for pending in &mut state.pending_notifications {
        if pending.authority == key.authority {
          let mut flush_index = 0;
          while flush_index < pending.flush_ids.len() {
            if pending.flush_ids[flush_index] == key.flush_id {
              pending.flush_ids.remove(flush_index);
            } else {
              flush_index += 1;
            }
          }
        }
      }
      let mut ready = Vec::new();
      let mut index = 0;
      while index < state.pending_notifications.len() {
        if state.pending_notifications[index].flush_ids.is_empty() {
          ready.push(state.pending_notifications.remove(index));
        } else {
          index += 1;
        }
      }
      for waiter in &state.shutdown_waiters {
        waiter.observe(&key);
      }
      state.shutdown_waiters.retain(|waiter| !waiter.is_complete());
      ready
    };
    for pending in ready {
      self.enqueue_pending_outbound(event_sender, pending);
    }
  }

  fn enqueue_outbound(
    &self,
    event_sender: &Sender<RemoteEvent>,
    authority: TransportEndpoint,
    envelope: OutboundEnvelope,
    now_ms: u64,
  ) -> bool {
    let event = RemoteEvent::OutboundEnqueued { authority, envelope: Box::new(envelope), now_ms };
    match event_sender.try_send(event) {
      | Ok(()) => true,
      | Err(TrySendError::Full(event)) => {
        self.defer_outbound_event(event_sender, event);
        true
      },
      | Err(TrySendError::Closed(_)) => {
        tracing::warn!("remote watch notification event queue is closed");
        true
      },
    }
  }

  fn enqueue_pending_outbound(&self, event_sender: &Sender<RemoteEvent>, pending: PendingNotification) {
    match event_sender.try_reserve() {
      | Ok(permit) => {
        let PendingNotification { authority, envelope, now_ms, .. } = pending;
        permit.send(RemoteEvent::OutboundEnqueued { authority, envelope: Box::new(envelope), now_ms });
      },
      | Err(TrySendError::Full(())) => {
        let PendingNotification { authority, envelope, now_ms, .. } = pending;
        self.defer_outbound_event(event_sender, RemoteEvent::OutboundEnqueued {
          authority,
          envelope: Box::new(envelope),
          now_ms,
        });
      },
      | Err(TrySendError::Closed(_)) => {
        tracing::warn!("remote watch notification event queue is closed");
      },
    }
  }

  fn defer_outbound_event(&self, event_sender: &Sender<RemoteEvent>, event: RemoteEvent) {
    tracing::warn!("remote watch notification event queue is full");
    let retry_sender = self.retry_sender_for(event_sender);
    match retry_sender.try_send(event) {
      | Ok(()) => {},
      | Err(TrySendError::Full(_)) => {
        tracing::warn!("remote watch notification retry queue is full");
      },
      | Err(TrySendError::Closed(_)) => {
        tracing::warn!("remote watch notification retry queue is closed");
      },
    }
  }

  fn retry_sender_for(&self, event_sender: &Sender<RemoteEvent>) -> Sender<RemoteEvent> {
    let mut state = self.inner.lock().expect(FLUSH_GATE_LOCK_POISONED);
    if let Some(retry_sender) = &state.retry_sender {
      return retry_sender.clone();
    }
    let (retry_sender, mut retry_receiver) = mpsc::channel(RETRY_QUEUE_CAPACITY);
    let sender = event_sender.clone();
    let _retry_task = tokio::spawn(async move {
      while let Some(event) = retry_receiver.recv().await {
        if sender.send(event).await.is_err() {
          tracing::warn!("remote watch notification event queue is closed");
          break;
        }
      }
    });
    state.retry_sender = Some(retry_sender.clone());
    retry_sender
  }
}

impl Default for StdFlushGate {
  fn default() -> Self {
    Self::new()
  }
}

impl StdFlushWaitHandle {
  fn new(pending: Vec<FlushKey>) -> Self {
    Self { inner: Arc::new(Mutex::new(StdFlushWaitState { pending })), notify: Arc::new(Notify::new()) }
  }

  fn observe(&self, key: &FlushKey) {
    let mut state = self.inner.lock().expect(FLUSH_GATE_LOCK_POISONED);
    state.pending.retain(|pending| pending != key);
    if state.pending.is_empty() {
      self.notify.notify_waiters();
    }
  }

  fn is_complete(&self) -> bool {
    self.inner.lock().expect(FLUSH_GATE_LOCK_POISONED).pending.is_empty()
  }

  pub(super) async fn wait(self, timeout: Duration) -> bool {
    let notified = self.notify.notified();
    tokio::pin!(notified);
    if self.is_complete() {
      return true;
    }
    tokio::select! {
      () = &mut notified => self.is_complete(),
      () = sleep(timeout) => self.is_complete(),
    }
  }
}

pub(super) fn schedule_flush_timers(
  event_sender: &Sender<RemoteEvent>,
  monotonic_epoch: Instant,
  timers: &[RemoteFlushTimer],
) {
  for timer in timers {
    let sender = event_sender.clone();
    let authority = timer.authority().clone();
    let flush_id = timer.flush_id();
    let deadline_ms = timer.deadline_ms();
    let delay_ms = deadline_ms.saturating_sub(std_instant_elapsed_millis(monotonic_epoch));
    let _timer_task = tokio::spawn(async move {
      sleep(Duration::from_millis(delay_ms)).await;
      let now_ms = std_instant_elapsed_millis(monotonic_epoch);
      match sender.send(RemoteEvent::FlushTimerFired { authority, flush_id, now_ms }).await {
        | Ok(()) => {},
        | Err(error) => tracing::warn!(?error, "flush timeout event delivery failed"),
      }
    });
  }
}
