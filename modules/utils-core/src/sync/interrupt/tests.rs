#[allow(unused_imports)]
#[cfg(not(feature = "interrupt-cortex-m"))]
use super::CriticalSectionInterruptPolicy;
use super::{InterruptContextPolicy, NeverInterruptPolicy};

#[test]
fn never_policy_allows_blocking() {
  assert!(NeverInterruptPolicy::check_blocking_allowed().is_ok());
}

#[cfg(not(feature = "interrupt-cortex-m"))]
#[test]
fn critical_section_policy_allows_blocking_without_detection() {
  assert!(CriticalSectionInterruptPolicy::check_blocking_allowed().is_ok());
}
