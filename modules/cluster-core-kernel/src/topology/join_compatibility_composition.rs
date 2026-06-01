//! Ordered join compatibility checker composition.

#[cfg(test)]
#[path = "join_compatibility_composition_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use crate::{ClusterExtensionConfig, ConfigValidation, JoinConfigCompatChecker};

/// Composes multiple join compatibility checkers without dropping rejection reasons.
pub struct JoinCompatibilityComposition<'a> {
  checkers: Vec<&'a dyn JoinConfigCompatChecker>,
}

impl<'a> JoinCompatibilityComposition<'a> {
  /// Creates a composition from ordered join compatibility checkers.
  #[must_use]
  pub fn new(checkers: Vec<&'a dyn JoinConfigCompatChecker>) -> Self {
    Self { checkers }
  }
}

impl JoinConfigCompatChecker for JoinCompatibilityComposition<'_> {
  fn check_join_compatibility(&self, joining: &ClusterExtensionConfig) -> ConfigValidation {
    let mut combined_reason = String::new();

    for checker in &self.checkers {
      let ConfigValidation::Incompatible { reason } = checker.check_join_compatibility(joining) else {
        continue;
      };
      if !combined_reason.is_empty() {
        combined_reason.push_str("; ");
      }
      combined_reason.push_str(&reason);
    }

    if combined_reason.is_empty() {
      ConfigValidation::Compatible
    } else {
      ConfigValidation::Incompatible { reason: combined_reason }
    }
  }
}
