//! Standard-library [`ActorSystemConfig`] factory with a monotonic mailbox clock
//! pre-installed.

use fraktor_actor_core_rs::actor::{scheduler::tick_driver::TickDriver, setup::ActorSystemConfig};

use crate::std::time::std_monotonic_mailbox_clock;

/// Creates an [`ActorSystemConfig`] whose mailbox lock bundle carries the
/// std monotonic clock, so every system built from this config performs
/// throughput deadline enforcement (Pekko `Mailbox.scala:263-275`) using
/// [`std::time::Instant`].
///
/// Prefer this factory over `ActorSystemConfig::new(driver)` for std-backed
/// production systems. Callers that start from an existing
/// [`ActorSystemConfig`] can simply chain
/// `.with_mailbox_clock(std_monotonic_mailbox_clock())` instead.
#[must_use]
pub fn std_actor_system_config(driver: impl TickDriver + 'static) -> ActorSystemConfig {
  ActorSystemConfig::new(driver).with_mailbox_clock(std_monotonic_mailbox_clock())
}
