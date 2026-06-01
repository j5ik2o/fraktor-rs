use alloc::{string::ToString, vec, vec::Vec};

use super::JoinCompatibilityComposition;
use crate::{ClusterExtensionConfig, ConfigValidation, JoinConfigCompatChecker};

struct IncompatibleChecker {
  reason: &'static str,
}

impl JoinConfigCompatChecker for IncompatibleChecker {
  fn check_join_compatibility(&self, _joining: &ClusterExtensionConfig) -> ConfigValidation {
    ConfigValidation::Incompatible { reason: self.reason.to_string() }
  }
}

struct CompatibleChecker;

impl JoinConfigCompatChecker for CompatibleChecker {
  fn check_join_compatibility(&self, _joining: &ClusterExtensionConfig) -> ConfigValidation {
    ConfigValidation::Compatible
  }
}

#[test]
fn join_compatibility_composition_preserves_all_incompatible_reasons() {
  let first = IncompatibleChecker { reason: "first checker rejected" };
  let compatible = CompatibleChecker;
  let second = IncompatibleChecker { reason: "second checker rejected" };
  let checkers: Vec<&dyn JoinConfigCompatChecker> = vec![&first, &compatible, &second];
  let composition = JoinCompatibilityComposition::new(checkers);

  let validation = composition.check_join_compatibility(&ClusterExtensionConfig::new());

  let ConfigValidation::Incompatible { reason } = validation else {
    panic!("composition should reject when any checker rejects");
  };
  assert!(reason.contains("first checker rejected"));
  assert!(reason.contains("second checker rejected"));
}
