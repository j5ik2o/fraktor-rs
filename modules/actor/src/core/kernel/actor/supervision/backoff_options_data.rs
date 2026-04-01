//! Shared implementation for backoff supervisor option types.

use alloc::string::String;
use core::time::Duration;

use super::{BackoffSupervisorStrategy, SupervisorStrategy};
use crate::core::kernel::actor::props::Props;

#[derive(Clone)]
pub(super) struct BackoffOptionsData {
  child_props:         Props,
  child_name:          String,
  strategy:            BackoffSupervisorStrategy,
  auto_reset:          Option<Duration>,
  manual_reset:        bool,
  supervisor_strategy: Option<SupervisorStrategy>,
  max_retries:         u32,
}

impl BackoffOptionsData {
  #[must_use]
  pub(super) const fn new(child_props: Props, child_name: String, strategy: BackoffSupervisorStrategy) -> Self {
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

  pub(super) const fn set_auto_reset(&mut self, duration: Duration) {
    self.auto_reset = Some(duration);
  }

  pub(super) const fn enable_manual_reset(&mut self) {
    self.manual_reset = true;
  }

  #[must_use]
  pub(super) fn with_supervisor_strategy(mut self, strategy: SupervisorStrategy) -> Self {
    self.supervisor_strategy = Some(strategy);
    self
  }

  pub(super) const fn set_max_retries(&mut self, count: u32) {
    self.max_retries = count;
  }

  #[must_use]
  pub(super) const fn auto_reset(&self) -> Option<Duration> {
    self.auto_reset
  }

  #[must_use]
  pub(super) const fn manual_reset(&self) -> bool {
    self.manual_reset
  }

  #[must_use]
  pub(super) const fn supervisor_strategy(&self) -> Option<&SupervisorStrategy> {
    self.supervisor_strategy.as_ref()
  }

  #[must_use]
  pub(super) const fn max_retries(&self) -> u32 {
    self.max_retries
  }

  #[must_use]
  pub(super) fn child_name(&self) -> &str {
    &self.child_name
  }

  #[must_use]
  pub(super) const fn strategy(&self) -> &BackoffSupervisorStrategy {
    &self.strategy
  }

  #[must_use]
  pub(super) const fn child_props(&self) -> &Props {
    &self.child_props
  }
}
