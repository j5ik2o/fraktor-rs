//! Options for creating a backoff supervisor that restarts its child on stop.

use alloc::string::String;
use core::time::Duration;

use super::{BackoffSupervisorStrategy, SupervisorStrategy};
use crate::core::kernel::actor::props::Props;

#[cfg(test)]
mod tests;

/// Options for creating a backoff supervisor that restarts its child on stop.
///
/// Corresponds to Pekko's `BackoffOnStopOptions`.
#[derive(Clone)]
pub struct BackoffOnStopOptions {
  child_props:         Props,
  child_name:          String,
  strategy:            BackoffSupervisorStrategy,
  auto_reset:          Option<Duration>,
  manual_reset:        bool,
  supervisor_strategy: Option<SupervisorStrategy>,
  max_retries:         u32,
}

impl BackoffOnStopOptions {
  /// Creates new options with the required fields.
  ///
  /// Defaults: `auto_reset = None`, `manual_reset = false`,
  /// `supervisor_strategy = None`, `max_retries = 0` (unlimited).
  #[must_use]
  pub const fn new(child_props: Props, child_name: String, strategy: BackoffSupervisorStrategy) -> Self {
    Self {
      child_props,
      child_name,
      strategy,
      auto_reset: None,
      manual_reset: false,
      supervisor_strategy: None,
      max_retries: 0,
    }
  }

  /// Sets the duration after which the backoff counter resets automatically.
  #[must_use]
  pub const fn with_auto_reset(mut self, duration: Duration) -> Self {
    self.auto_reset = Some(duration);
    self
  }

  /// Enables manual reset mode for the backoff counter.
  #[must_use]
  pub const fn with_manual_reset(mut self) -> Self {
    self.manual_reset = true;
    self
  }

  /// Sets the supervisor strategy used for the child actor.
  #[must_use]
  pub fn with_supervisor_strategy(mut self, strategy: SupervisorStrategy) -> Self {
    self.supervisor_strategy = Some(strategy);
    self
  }

  /// Sets the maximum number of retries before giving up. 0 means unlimited.
  #[must_use]
  pub const fn with_max_retries(mut self, count: u32) -> Self {
    self.max_retries = count;
    self
  }

  /// Returns the auto-reset duration, if configured.
  #[must_use]
  pub const fn auto_reset(&self) -> Option<Duration> {
    self.auto_reset
  }

  /// Returns whether manual reset mode is enabled.
  #[must_use]
  pub const fn manual_reset(&self) -> bool {
    self.manual_reset
  }

  /// Returns the supervisor strategy, if configured.
  #[must_use]
  pub const fn supervisor_strategy(&self) -> Option<&SupervisorStrategy> {
    self.supervisor_strategy.as_ref()
  }

  /// Returns the maximum number of retries. 0 means unlimited.
  #[must_use]
  pub const fn max_retries(&self) -> u32 {
    self.max_retries
  }

  /// Returns the child actor name.
  #[must_use]
  pub fn child_name(&self) -> &str {
    &self.child_name
  }

  /// Returns the backoff supervisor strategy.
  #[must_use]
  pub const fn strategy(&self) -> &BackoffSupervisorStrategy {
    &self.strategy
  }

  /// Returns the child actor props.
  #[must_use]
  pub const fn child_props(&self) -> &Props {
    &self.child_props
  }
}
