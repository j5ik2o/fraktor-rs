mod critical_section_interrupt_policy;
mod never_interrupt_policy;
#[cfg(test)]
mod tests;

pub use critical_section_interrupt_policy::CriticalSectionInterruptPolicy;
pub use never_interrupt_policy::NeverInterruptPolicy;

use crate::v2::sync::SharedError;

/// Policy interface for determining whether blocking operations are permitted in the
/// current execution context.
pub trait InterruptContextPolicy {
  /// Checks whether blocking operations are allowed.
  fn check_blocking_allowed() -> Result<(), SharedError>;
}
