//! Embassy monotonic time helpers.

use alloc::boxed::Box;
use core::time::Duration;

use embassy_time::Instant;
use fraktor_actor_core_kernel_rs::dispatch::mailbox::MailboxClock;
use fraktor_utils_core_rs::sync::ArcShared;

/// Creates a mailbox clock backed by [`embassy_time::Instant`].
#[must_use]
pub fn embassy_monotonic_mailbox_clock() -> MailboxClock {
  ArcShared::from_boxed(Box::new(|| Duration::from_micros(Instant::now().as_micros())))
}
