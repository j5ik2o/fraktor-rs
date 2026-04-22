//! Test-only constructors for empty actor systems backed by [`TestTickDriver`].

use fraktor_actor_core_rs::core::kernel::{actor::setup::ActorSystemConfig, system::ActorSystem};

use crate::std::{tick_driver::TestTickDriver, time::std_monotonic_mailbox_clock};

/// Creates an empty actor system without any guardian using the default test tick driver.
///
/// Equivalent to calling [`new_empty_actor_system_with`] with an identity configurator.
///
/// # Panics
///
/// Panics if the default test-support configuration fails to build.
#[must_use]
pub fn new_empty_actor_system() -> ActorSystem {
  new_empty_actor_system_with(|config| config)
}

/// Creates an empty actor system without any guardian, allowing the caller to customize the
/// [`ActorSystemConfig`] before the system state is built.
///
/// The system is backed by [`TestTickDriver`] (std-thread driven, suitable for deterministic
/// integration tests). Internally calls [`ActorSystem::new_started_from_config`] which marks
/// the root as started.
///
/// # Panics
///
/// Panics if the resulting configuration fails to build the underlying system state.
#[must_use]
pub fn new_empty_actor_system_with<F>(configure: F) -> ActorSystem
where
  F: FnOnce(ActorSystemConfig) -> ActorSystemConfig, {
  // Install the std monotonic mailbox clock on the config so every mailbox
  // constructed under this system observes real elapsed time when enforcing
  // the throughput deadline (Pekko `Mailbox.scala:263-275`). Wiring this at
  // the config level means the clock flows through `ActorSystem::create_*` /
  // `new_started_from_config` uniformly, not only this test-support factory.
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_mailbox_clock(std_monotonic_mailbox_clock());
  let config = configure(config);
  match ActorSystem::new_started_from_config(config) {
    | Ok(system) => system,
    | Err(error) => panic!("test-support config failed to build in new_empty_actor_system_with: {error:?}"),
  }
}
