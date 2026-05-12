#[cfg(test)]
#[path = "cancellation_strategy_kind_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};

/// Strategy applied when a stage receives a cancellation signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationStrategyKind {
  /// Complete the stage normally on cancellation.
  CompleteStage,
  /// Fail the stage on cancellation.
  FailStage,
  /// Propagate the failure upstream.
  PropagateFailure,
}

impl Attribute for CancellationStrategyKind {
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

impl MandatoryAttribute for CancellationStrategyKind {}
