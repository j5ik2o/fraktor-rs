use super::NeverInterruptPolicy;
use crate::core::sync::interrupt::InterruptContextPolicy;

#[test]
fn never_interrupt_policy_check_blocking_allowed() {
  let result = NeverInterruptPolicy::check_blocking_allowed();
  assert!(result.is_ok());
}

#[test]
fn never_interrupt_policy_is_struct() {
  fn assert_exists(_: &NeverInterruptPolicy) {}
  assert_exists(&NeverInterruptPolicy);
}
