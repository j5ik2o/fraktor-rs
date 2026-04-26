//! Handshake-timeout driver: arms a `tokio::time::sleep` and notifies the
//! pure `Association` when the deadline expires.

use core::time::Duration;
use std::time::Instant;

use fraktor_remote_core_rs::domain::extension::EventPublisher;
use tokio::task::JoinHandle;

use crate::std::association_runtime::{apply_effects_in_place, association_shared::AssociationShared};

/// Drives the handshake-timeout deadline for one `Association`.
///
/// `HandshakeDriver::arm` spawns a tokio task that sleeps for `timeout` and
/// then calls `Association::handshake_timed_out`, passing the elapsed
/// monotonic millis since `started_at`. The driver intentionally derives
/// `now_ms` from `Instant::now()`'s difference (not from `SystemTime`), so
/// wall-clock jumps cannot trigger spurious timeouts.
#[derive(Default)]
pub struct HandshakeDriver {
  task: Option<JoinHandle<()>>,
}

impl HandshakeDriver {
  /// Creates a new, idle driver.
  #[must_use]
  pub const fn new() -> Self {
    Self { task: None }
  }

  /// Returns `true` when a timeout task is currently armed.
  #[must_use]
  pub fn is_armed(&self) -> bool {
    self.task.as_ref().is_some_and(|t| !t.is_finished())
  }

  /// Arms the driver to fire after `timeout` and notify `shared`.
  ///
  /// `started_at` is a `std::time::Instant` captured at handshake start; the
  /// driver computes the elapsed monotonic millis at firing time. Re-arming
  /// before the previous task fires aborts the old task.
  pub fn arm(
    &mut self,
    shared: AssociationShared,
    started_at: Instant,
    timeout: Duration,
    event_publisher: EventPublisher,
  ) {
    if let Some(handle) = self.task.take() {
      handle.abort();
    }
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
    self.task = Some(task);
  }

  /// Cancels any pending timeout task.
  pub fn cancel(&mut self) {
    if let Some(handle) = self.task.take() {
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
