//! Handshake driver for timeout, retry, injection, and liveness probes.

use core::time::Duration;
use std::time::Instant;

use fraktor_remote_core_rs::core::{
  address::{Address, UniqueAddress},
  association::AssociationState,
  extension::EventPublisher,
  transport::TransportError,
  wire::{HandshakePdu, HandshakeReq},
};
use tokio::task::JoinHandle;

use crate::std::association_runtime::{apply_effects_in_place, association_shared::AssociationShared};

/// Drives handshake control tasks for one `Association`.
///
/// The driver intentionally derives timeout `now_ms` from `Instant::now()`'s
/// difference (not from `SystemTime`), so wall-clock jumps cannot trigger
/// spurious timeouts.
#[derive(Default)]
pub struct HandshakeDriver {
  tasks: Vec<JoinHandle<()>>,
}

impl HandshakeDriver {
  /// Creates a new, idle driver.
  #[must_use]
  pub const fn new() -> Self {
    Self { tasks: Vec::new() }
  }

  /// Returns `true` when at least one handshake control task is currently armed.
  #[must_use]
  pub fn is_armed(&self) -> bool {
    self.tasks.iter().any(|task| !task.is_finished())
  }

  /// Arms the driver to fire after `timeout` and notify `shared`.
  ///
  /// `started_at` is a `std::time::Instant` captured at handshake start; the
  /// driver computes the elapsed monotonic millis at firing time.
  pub fn arm(
    &mut self,
    shared: AssociationShared,
    started_at: Instant,
    timeout: Duration,
    event_publisher: EventPublisher,
  ) {
    let task = tokio::spawn(async move {
      tokio::time::sleep(timeout).await;
      let now_ms = monotonic_millis_since(started_at);
      shared.with_write(|assoc| {
        let effects = assoc.handshake_timed_out(now_ms, None);
        // Discarding `effects` here would silently drop the `Gated`
        // lifecycle event and the `DiscardEnvelopes` notice that contains
        // every envelope buffered during the handshake. apply_effects_in_place
        // publishes the lifecycle event and logs the discard so the operator
        // can observe the loss.
        apply_effects_in_place(assoc, effects, &event_publisher);
      });
    });
    self.tasks.push(task);
  }

  /// Arms periodic handshake request retry while the association is handshaking.
  pub fn arm_retry<F>(
    &mut self,
    shared: AssociationShared,
    local: UniqueAddress,
    remote: Address,
    interval: Duration,
    send_handshake: F,
  ) where
    F: FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static, {
    self.tasks.push(spawn_periodic_handshake_sender(
      shared,
      local,
      remote,
      interval,
      send_handshake,
      should_retry,
      "retry handshake request failed",
    ));
  }

  /// Arms periodic handshake injection while the association is active.
  pub fn arm_inject<F>(
    &mut self,
    shared: AssociationShared,
    local: UniqueAddress,
    remote: Address,
    interval: Duration,
    send_handshake: F,
  ) where
    F: FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static, {
    self.tasks.push(spawn_periodic_handshake_sender(
      shared,
      local,
      remote,
      interval,
      send_handshake,
      should_inject,
      "inject handshake request failed",
    ));
  }

  /// Arms periodic liveness probes for an idle active association.
  pub fn arm_liveness_probe<F, N>(
    &mut self,
    shared: AssociationShared,
    local: UniqueAddress,
    remote: Address,
    interval: Duration,
    now_ms_provider: N,
    send_handshake: F,
  ) where
    F: FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static,
    N: Fn() -> u64 + Send + 'static, {
    let task = tokio::spawn(async move {
      let mut send_handshake = send_handshake;
      loop {
        tokio::time::sleep(interval).await;
        let now_ms = now_ms_provider();
        let due = shared.with_write(|assoc| assoc.is_liveness_probe_due(now_ms, interval.as_millis() as u64));
        if due {
          let pdu = HandshakePdu::Req(HandshakeReq::new(local.clone(), remote.clone()));
          match send_handshake(&remote, pdu) {
            | Ok(()) => shared.with_write(|assoc| assoc.record_handshake_activity(now_ms)),
            | Err(err) => tracing::warn!(remote = %remote, ?err, "liveness handshake request failed"),
          }
        }
      }
    });
    self.tasks.push(task);
  }

  /// Cancels all pending handshake control tasks.
  pub fn cancel(&mut self) {
    for handle in self.tasks.drain(..) {
      handle.abort();
    }
  }
}

/// Computes the monotonic millis elapsed between `started_at` and `now`.
///
/// This is the **only** place in the adapter that materialises an
/// `Instant`-derived `u64` for the pure core layer (per design Decision 7).
fn monotonic_millis_since(started_at: Instant) -> u64 {
  started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn spawn_periodic_handshake_sender<F, P>(
  shared: AssociationShared,
  local: UniqueAddress,
  remote: Address,
  interval: Duration,
  send_handshake: F,
  should_send: P,
  failure_message: &'static str,
) -> JoinHandle<()>
where
  F: FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static,
  P: Fn(&AssociationState) -> bool + Send + 'static, {
  tokio::spawn(async move {
    let mut send_handshake = send_handshake;
    loop {
      tokio::time::sleep(interval).await;
      let should_send = shared.with_write(|assoc| should_send(assoc.state()));
      if should_send {
        let pdu = HandshakePdu::Req(HandshakeReq::new(local.clone(), remote.clone()));
        if let Err(err) = send_handshake(&remote, pdu) {
          tracing::warn!(remote = %remote, ?err, failure_message);
        }
      }
    }
  })
}

fn should_retry(state: &AssociationState) -> bool {
  matches!(state, AssociationState::Handshaking { .. })
}

fn should_inject(state: &AssociationState) -> bool {
  matches!(state, AssociationState::Active { .. })
}
