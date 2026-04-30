//! Options for creating a backoff supervisor that restarts its child on stop.

use alloc::string::String;
use core::time::Duration;

use super::{BackoffSupervisorStrategy, SupervisorStrategy, backoff_options_data::BackoffOptionsData};
use crate::core::kernel::actor::props::Props;

#[cfg(test)]
mod tests;

/// Options for creating a backoff supervisor that restarts its child on stop.
///
/// Corresponds to Pekko's `BackoffOnStopOptions`.
///
/// This intentionally mirrors [`BackoffOnFailureOptions`](super::BackoffOnFailureOptions)
/// and shares storage through [`BackoffOptionsData`]. Keep the public option
/// types separate unless future fields or behavior diverge enough to make a
/// shared abstraction simpler than the duplicated API surface.
#[derive(Clone)]
pub struct BackoffOnStopOptions {
  inner: BackoffOptionsData,
}

impl BackoffOnStopOptions {
  /// Creates new options with the required fields.
  ///
  /// Defaults: `auto_reset = None`, `manual_reset = false`,
  /// `supervisor_strategy = None`, `max_retries = 0` (unlimited).
  #[must_use]
  pub const fn new(child_props: Props, child_name: String, strategy: BackoffSupervisorStrategy) -> Self {
    Self { inner: BackoffOptionsData::new(child_props, child_name, strategy) }
  }

  /// Sets the duration after which the backoff counter resets automatically.
  #[must_use]
  pub const fn with_auto_reset(mut self, duration: Duration) -> Self {
    self.inner.set_auto_reset(duration);
    self
  }

  /// Enables manual reset mode for the backoff counter.
  #[must_use]
  pub const fn with_manual_reset(mut self) -> Self {
    self.inner.enable_manual_reset();
    self
  }

  /// Sets the supervisor strategy used for the child actor.
  #[must_use]
  pub fn with_supervisor_strategy(mut self, strategy: SupervisorStrategy) -> Self {
    self.inner = self.inner.with_supervisor_strategy(strategy);
    self
  }

  /// Sets the maximum number of retries before giving up. 0 means unlimited.
  #[must_use]
  pub const fn with_max_retries(mut self, count: u32) -> Self {
    self.inner.set_max_retries(count);
    self
  }

  /// Returns the auto-reset duration, if configured.
  #[must_use]
  pub const fn auto_reset(&self) -> Option<Duration> {
    self.inner.auto_reset()
  }

  /// Returns whether manual reset mode is enabled.
  #[must_use]
  pub const fn manual_reset(&self) -> bool {
    self.inner.manual_reset()
  }

  /// Returns the supervisor strategy, if configured.
  #[must_use]
  pub const fn supervisor_strategy(&self) -> Option<&SupervisorStrategy> {
    self.inner.supervisor_strategy()
  }

  /// Returns the maximum number of retries. 0 means unlimited.
  #[must_use]
  pub const fn max_retries(&self) -> u32 {
    self.inner.max_retries()
  }

  /// Returns the child actor name.
  #[must_use]
  pub fn child_name(&self) -> &str {
    self.inner.child_name()
  }

  /// Returns the backoff supervisor strategy.
  #[must_use]
  pub const fn strategy(&self) -> &BackoffSupervisorStrategy {
    self.inner.strategy()
  }

  /// Returns the child actor props.
  #[must_use]
  pub const fn child_props(&self) -> &Props {
    self.inner.child_props()
  }
}
