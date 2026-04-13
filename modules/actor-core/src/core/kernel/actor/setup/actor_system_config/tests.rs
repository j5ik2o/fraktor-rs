use core::time::Duration;

use crate::core::kernel::{
  actor::{actor_path::GuardianKind as PathGuardianKind, setup::ActorSystemConfig},
  dispatch::dispatcher::DEFAULT_DISPATCHER_ID,
  pattern::{CircuitBreaker, CircuitBreakerShared, CircuitBreakerSharedFactory, CircuitBreakerState, Clock},
  system::remote::RemotingConfig,
};

struct NoopActor;

impl crate::core::kernel::actor::Actor for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut crate::core::kernel::actor::ActorContext<'_>,
    _message: crate::core::kernel::actor::messaging::AnyMessageView<'_>,
  ) -> Result<(), crate::core::kernel::actor::error::ActorError> {
    Ok(())
  }
}

#[derive(Clone)]
struct FakeClock;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FakeInstant(u64);

impl Clock for FakeClock {
  type Instant = FakeInstant;

  fn now(&self) -> Self::Instant {
    FakeInstant(0)
  }

  fn elapsed_since(&self, _earlier: Self::Instant) -> Duration {
    Duration::ZERO
  }
}

#[test]
fn test_actor_system_config_default() {
  let config = ActorSystemConfig::default();
  assert_eq!(config.system_name(), "default-system");
  assert_eq!(config.default_guardian(), PathGuardianKind::User);
  assert!(config.remoting_config().is_none());
}

#[test]
fn test_actor_system_config_with_system_name() {
  let config = ActorSystemConfig::default().with_system_name("test-system");
  assert_eq!(config.system_name(), "test-system");
}

#[test]
fn test_actor_system_config_with_default_guardian() {
  let config = ActorSystemConfig::default().with_default_guardian(PathGuardianKind::System);
  assert_eq!(config.default_guardian(), PathGuardianKind::System);
}

#[test]
fn test_actor_system_config_with_remoting() {
  let remoting = RemotingConfig::default().with_canonical_host("localhost").with_canonical_port(2552);

  let config = ActorSystemConfig::default().with_remoting_config(remoting);

  assert!(config.remoting_config().is_some());
  let remoting_cfg = config.remoting_config().unwrap();
  assert_eq!(remoting_cfg.canonical_host(), "localhost");
  assert_eq!(remoting_cfg.canonical_port(), Some(2552));
}

#[test]
fn test_remoting_config_quarantine_duration() {
  let custom_duration = Duration::from_secs(1800);
  let remoting = RemotingConfig::default().with_quarantine_duration(custom_duration);

  assert_eq!(remoting.quarantine_duration(), custom_duration);
}

#[test]
fn test_remoting_config_defaults() {
  let remoting = RemotingConfig::default();

  assert_eq!(remoting.canonical_host(), "localhost");
  assert_eq!(remoting.canonical_port(), None);
  assert_eq!(remoting.quarantine_duration(), Duration::from_secs(5 * 24 * 3600));
}

#[test]
#[should_panic(expected = "quarantine duration must be >= 1 second")]
fn test_remoting_config_rejects_short_quarantine() {
  drop(RemotingConfig::default().with_quarantine_duration(Duration::from_millis(999)));
}

#[test]
fn test_actor_system_config_default_resolves_default_dispatcher() {
  let config = ActorSystemConfig::default();
  assert!(
    config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok(),
    "ActorSystemConfig::default() should seed the default dispatcher entry"
  );
}

struct BuiltinCircuitBreakerFactory;

impl CircuitBreakerSharedFactory<FakeClock> for BuiltinCircuitBreakerFactory {
  fn create_circuit_breaker_shared(&self, cb: CircuitBreaker<FakeClock>) -> CircuitBreakerShared<FakeClock> {
    CircuitBreakerShared::new(cb)
  }
}

#[test]
fn test_actor_system_config_circuit_breaker_shared_factory_uses_registered_provider() {
  let config =
    ActorSystemConfig::default().with_circuit_breaker_shared_factory::<FakeClock, _>(BuiltinCircuitBreakerFactory);

  let shared = config
    .circuit_breaker_shared_factory::<FakeClock>()
    .expect("circuit breaker shared factory should be registered for FakeClock")
    .create_circuit_breaker_shared(CircuitBreaker::new_with_clock(2, Duration::from_secs(1), FakeClock));

  assert_eq!(shared.state(), CircuitBreakerState::Closed);
}
