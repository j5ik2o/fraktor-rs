#[cfg(test)]
#[path = "stream_subscription_timeout_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};
use crate::stream_subscription_timeout_termination_mode::StreamSubscriptionTimeoutTerminationMode;

/// Configures stream subscription-timeout semantics.
///
/// Mirrors Pekko's `StreamSubscriptionTimeout(timeout, mode)` settings
/// attribute. The timeout is expressed in scheduler ticks because
/// `stream-core-kernel` is `no_std` and does not depend on `core::time::Duration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSubscriptionTimeout {
  /// Subscription timeout in scheduler ticks.
  pub timeout_ticks:    u32,
  /// Termination mode applied when the subscription timeout fires.
  pub termination_mode: StreamSubscriptionTimeoutTerminationMode,
}

impl StreamSubscriptionTimeout {
  /// Creates a new subscription-timeout configuration.
  #[must_use]
  pub const fn new(timeout_ticks: u32, termination_mode: StreamSubscriptionTimeoutTerminationMode) -> Self {
    Self { timeout_ticks, termination_mode }
  }
}

impl Attribute for StreamSubscriptionTimeout {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> Box<dyn Attribute> {
    Box::new(*self)
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}

impl MandatoryAttribute for StreamSubscriptionTimeout {}
