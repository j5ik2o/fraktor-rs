use alloc::string::String;
use core::time::Duration;

use crate::core::kernel::actor::{
  props::Props,
  supervision::{BackoffOnFailureOptions, BackoffSupervisorStrategy, SupervisorStrategy, SupervisorStrategyKind},
};

struct DummyActor;

impl crate::core::kernel::actor::Actor for DummyActor {
  fn receive(
    &mut self,
    _context: &mut crate::core::kernel::actor::ActorContext<'_>,
    _message: crate::core::kernel::actor::messaging::AnyMessageView<'_>,
  ) -> Result<(), crate::core::kernel::actor::error::ActorError> {
    Ok(())
  }
}

fn default_strategy() -> BackoffSupervisorStrategy {
  BackoffSupervisorStrategy::new(Duration::from_millis(200), Duration::from_secs(20), 0.3)
}

fn child_props() -> Props {
  Props::from_fn(|| DummyActor)
}

// --- Construction ---

#[test]
fn new_creates_options_with_required_fields() {
  // Given: required parameters
  let props = child_props();
  let strategy = default_strategy();

  // When: constructing BackoffOnFailureOptions
  let options = BackoffOnFailureOptions::new(props, String::from("worker"), strategy);

  // Then: accessors return the expected values
  assert_eq!(options.child_name(), "worker");
  assert_eq!(options.strategy().min_backoff(), Duration::from_millis(200));
}

// --- Builder: auto_reset ---

#[test]
fn with_auto_reset_sets_duration() {
  // Given: default options
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());

  // When: setting auto reset
  let options = options.with_auto_reset(Duration::from_secs(45));

  // Then: auto_reset returns the configured duration
  assert_eq!(options.auto_reset(), Some(Duration::from_secs(45)));
}

#[test]
fn auto_reset_is_none_by_default() {
  // Given: default options
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());

  // Then: auto_reset is None
  assert!(options.auto_reset().is_none());
}

// --- Builder: manual_reset ---

#[test]
fn with_manual_reset_enables_manual_mode() {
  // Given: default options
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());

  // When: enabling manual reset
  let options = options.with_manual_reset();

  // Then: manual_reset is true
  assert!(options.manual_reset());
}

#[test]
fn manual_reset_is_false_by_default() {
  // Given: default options
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());

  // Then: manual_reset is false
  assert!(!options.manual_reset());
}

// --- Builder: supervisor_strategy ---

#[test]
fn with_supervisor_strategy_sets_strategy() {
  // Given: default options and a custom supervisor strategy
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());
  let custom_strategy =
    SupervisorStrategy::with_decider(|_| crate::core::kernel::actor::supervision::SupervisorDirective::Restart)
      .with_kind(SupervisorStrategyKind::OneForOne);

  // When: setting the supervisor strategy
  let options = options.with_supervisor_strategy(custom_strategy);

  // Then: supervisor_strategy is Some
  assert!(options.supervisor_strategy().is_some());
}

#[test]
fn supervisor_strategy_is_none_by_default() {
  // Given: default options
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());

  // Then: supervisor_strategy is None
  assert!(options.supervisor_strategy().is_none());
}

// --- Builder: max_retries ---

#[test]
fn with_max_retries_sets_value() {
  // Given: default options
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());

  // When: setting max retries
  let options = options.with_max_retries(3);

  // Then: max_retries returns the configured value
  assert_eq!(options.max_retries(), 3);
}

#[test]
fn max_retries_is_zero_by_default() {
  // Given: default options (0 = unlimited)
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy());

  // Then: max_retries is 0 (unlimited)
  assert_eq!(options.max_retries(), 0);
}

// --- Builder: chaining ---

#[test]
fn builder_methods_can_be_chained() {
  // Given: all builder methods chained together
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), default_strategy())
    .with_manual_reset()
    .with_max_retries(7);

  // Then: all values are set correctly
  assert!(options.manual_reset());
  assert_eq!(options.max_retries(), 7);
}

// --- Accessors ---

#[test]
fn strategy_accessor_returns_inner_strategy() {
  // Given: options with a specific strategy
  let strategy = BackoffSupervisorStrategy::new(Duration::from_secs(2), Duration::from_secs(120), 0.1);
  let options = BackoffOnFailureOptions::new(child_props(), String::from("child"), strategy);

  // Then: the strategy accessor returns the configured values
  assert_eq!(options.strategy().min_backoff(), Duration::from_secs(2));
  assert_eq!(options.strategy().max_backoff(), Duration::from_secs(120));
}

#[test]
fn child_name_accessor_returns_configured_name() {
  // Given: options with a specific child name
  let options = BackoffOnFailureOptions::new(child_props(), String::from("my-processor"), default_strategy());

  // Then: child_name returns the configured name
  assert_eq!(options.child_name(), "my-processor");
}
