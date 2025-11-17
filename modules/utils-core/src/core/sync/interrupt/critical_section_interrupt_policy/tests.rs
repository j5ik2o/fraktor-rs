use super::CriticalSectionInterruptPolicy;
use crate::core::sync::{SharedError, interrupt::InterruptContextPolicy};

#[test]
fn critical_section_interrupt_policy_check_blocking_allowed() {
  let result = CriticalSectionInterruptPolicy::check_blocking_allowed();
  assert!(result.is_ok() || matches!(result, Err(SharedError::InterruptContext)));
}

#[test]
fn critical_section_interrupt_policy_is_struct() {
  fn assert_exists(_: &CriticalSectionInterruptPolicy) {}
  assert_exists(&CriticalSectionInterruptPolicy);
}
