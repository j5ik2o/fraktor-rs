#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::Attribute;

/// Remote subscription timeout for stream references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamRefSubscriptionTimeout {
  /// Subscription timeout in scheduler ticks.
  pub timeout_ticks: u32,
}

impl StreamRefSubscriptionTimeout {
  /// Creates a stream reference subscription timeout attribute.
  #[must_use]
  pub const fn new(timeout_ticks: u32) -> Self {
    Self { timeout_ticks }
  }
}

impl Attribute for StreamRefSubscriptionTimeout {
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
