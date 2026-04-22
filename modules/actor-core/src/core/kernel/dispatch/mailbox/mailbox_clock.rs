//! Monotonic clock injection for mailbox throughput deadline enforcement.
//!
//! The [`MailboxClock`] type alias wraps a shared closure returning the
//! current monotonic duration. It is passed to [`Mailbox`](super::Mailbox) via
//! [`MailboxSharedSet`](crate::core::kernel::system::shared_factory::MailboxSharedSet)
//! and consumed during [`Mailbox::run`](super::Mailbox::run) to compute the
//! throughput deadline and to evaluate it on each loop iteration.
//!
//! Pekko reference:
//! `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala:263-275`
//! (`System.nanoTime + throughputDeadlineTime.toNanos` and
//! `(System.nanoTime - deadlineNs) < 0`).
//!
//! # Contract
//!
//! The injected closure must return a **monotonic** duration — one that never
//! moves backwards regardless of wall-clock adjustments. Wall-clock (`SystemTime`)
//! implementations are not supported; deadline comparisons rely on the clock
//! being strictly non-decreasing.
//!
//! # Layer separation
//!
//! This type alias is defined in `core` (no_std) so that downstream crates can
//! reference `MailboxClock` without pulling in `std`. The default `Instant::now()`
//! based implementation lives in the std adaptor. `no_std` embedded targets
//! inject their own monotonic source via the same type alias.
//!
//! # Debug
//!
//! `Arc<dyn Fn() -> Duration + Send + Sync>` does not implement `Debug`. Any
//! struct holding an `Option<MailboxClock>` field that requires `Debug` must
//! provide a manual implementation that skips or stubs the clock field.

use alloc::sync::Arc;
use core::time::Duration;

/// Shared monotonic clock callback used by [`Mailbox`](super::Mailbox) to evaluate
/// the throughput deadline on each loop iteration.
///
/// See the module-level documentation for the full contract.
pub type MailboxClock = Arc<dyn Fn() -> Duration + Send + Sync>;
