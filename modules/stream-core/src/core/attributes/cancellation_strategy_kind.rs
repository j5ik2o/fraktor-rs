#[cfg(test)]
mod tests;

use core::any::Any;

use super::Attribute;

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

  fn clone_box(&self) -> alloc::boxed::Box<dyn Attribute> {
    alloc::boxed::Box::new(*self)
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}
