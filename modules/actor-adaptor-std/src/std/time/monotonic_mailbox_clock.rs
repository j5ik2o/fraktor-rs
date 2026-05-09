//! Standard-library monotonic clock helper for mailbox throughput deadline.

extern crate std;

use alloc::boxed::Box;
use core::time::Duration;
use std::time::Instant;

use fraktor_actor_core_kernel_rs::dispatch::mailbox::MailboxClock;
use fraktor_utils_core_rs::core::sync::ArcShared;

/// Builds a [`MailboxClock`] backed by [`Instant::now`] for throughput
/// deadline enforcement.
///
/// The closure captures a single `start: Instant` at construction time and
/// returns `start.elapsed()` on each invocation. This yields a monotonic
/// [`Duration`] independent of wall-clock adjustments, satisfying Pekko
/// `System.nanoTime()` (`Mailbox.scala:263-275`) semantics.
///
/// The closure intentionally does **not** capture any `Arc<ActorSystem>`
/// or weak reference so that dropping the surrounding system cannot create
/// a reference cycle.
#[must_use]
pub fn std_monotonic_mailbox_clock() -> MailboxClock {
  let start = Instant::now();
  let closure: Box<dyn Fn() -> Duration + Send + Sync> = Box::new(move || -> Duration { start.elapsed() });
  ArcShared::from_boxed(closure)
}
