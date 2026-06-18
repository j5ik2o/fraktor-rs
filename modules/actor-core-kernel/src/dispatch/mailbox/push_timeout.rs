//! Helpers for Pekko-style bounded mailbox push timeouts.

use core::{hint::spin_loop, time::Duration};

use super::MailboxClock;

pub(crate) fn push_timeout_deadline(clock: Option<&MailboxClock>, timeout: Duration) -> Option<Duration> {
  clock.map(|now| now().saturating_add(timeout))
}

pub(crate) fn should_retry_after_full(clock: Option<&MailboxClock>, deadline: Option<Duration>) -> bool {
  match clock.zip(deadline) {
    | Some((now, deadline)) => now() < deadline,
    | None => false,
  }
}

pub(crate) fn spin_before_push_timeout_retry() {
  spin_loop();
}
