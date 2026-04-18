#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};

/// Policy controlling how cancellation is propagated into nested
/// materializations.
///
/// Pekko parity: `Attributes.NestedMaterializationCancellationPolicy`. The
/// `EAGER_CANCELLATION` / `PROPAGATE_TO_NESTED` constants correspond to the
/// Pekko singletons, and `DEFAULT` aliases `EAGER_CANCELLATION` following the
/// `Default = EagerCancellation` convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NestedMaterializationCancellationPolicy {
  propagate_to_nested_materialization: bool,
}

impl NestedMaterializationCancellationPolicy {
  /// Default policy aliasing [`EAGER_CANCELLATION`](Self::EAGER_CANCELLATION).
  pub const DEFAULT: Self = Self::EAGER_CANCELLATION;
  /// Cancellation is handled eagerly by the outer stage and is **not**
  /// forwarded to nested materializations.
  pub const EAGER_CANCELLATION: Self = Self { propagate_to_nested_materialization: false };
  /// Cancellation is forwarded to nested materializations so that they can
  /// react and shut down.
  pub const PROPAGATE_TO_NESTED: Self = Self { propagate_to_nested_materialization: true };

  /// Creates a policy with the given propagation flag.
  #[must_use]
  pub const fn new(propagate_to_nested_materialization: bool) -> Self {
    Self { propagate_to_nested_materialization }
  }

  /// Returns `true` when cancellation should be propagated to nested
  /// materializations.
  #[must_use]
  pub const fn propagate_to_nested_materialization(&self) -> bool {
    self.propagate_to_nested_materialization
  }
}

impl Attribute for NestedMaterializationCancellationPolicy {
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

impl MandatoryAttribute for NestedMaterializationCancellationPolicy {}
