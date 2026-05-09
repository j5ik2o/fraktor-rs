//! Test-only constructors for empty actor systems backed by [`TestTickDriver`].

use fraktor_actor_core_rs::{actor::setup::ActorSystemConfig, system::ActorSystem};

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
/// integration tests). Internally calls [`ActorSystem::create_started_from_config`] which marks
/// the root as started.
///
/// # Panics
///
/// Panics if the resulting configuration fails to build the underlying system state.
#[must_use]
pub fn new_empty_actor_system_with<F>(configure: F) -> ActorSystem
where
  F: FnOnce(ActorSystemConfig) -> ActorSystemConfig, {
  // std monotonic mailbox clock を config レベルで install することで、この
  // system 配下で `ActorCell::create` が構築するすべての mailbox が
  // throughput deadline 判定時に実経過時間を観測する (Pekko `Mailbox.scala:263-275`)。
  // config 経路に寄せることで、このテスト用 factory に限らず
  // `ActorSystem::create_*` / `create_started_from_config` 全般で同じ clock が届く。
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_mailbox_clock(std_monotonic_mailbox_clock());
  let config = configure(config);
  match ActorSystem::create_started_from_config(config) {
    | Ok(system) => system,
    | Err(error) => panic!("test-support config failed to build in new_empty_actor_system_with: {error:?}"),
  }
}
