mod critical_section_interrupt_policy;
mod never_interrupt_policy;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub(crate) use critical_section_interrupt_policy::CriticalSectionInterruptPolicy;
pub(crate) use never_interrupt_policy::NeverInterruptPolicy;

use crate::core::sync::SharedError;

/// Policy interface for determining whether blocking operations are permitted in the
/// current execution context.
#[allow(dead_code)]
pub(crate) trait InterruptContextPolicy {
  /// Checks whether blocking operations are allowed.
  ///
  /// # Errors
  ///
  /// Returns `SharedError::InterruptContext` when blocking is not permitted in the current
  /// execution context.
  fn check_blocking_allowed() -> Result<(), SharedError>;
}
