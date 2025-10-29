use super::InterruptContextPolicy;
use crate::sync::shared_error::SharedError;

/// Policy that never reports an active interrupt context.
pub struct NeverInterruptPolicy;

impl InterruptContextPolicy for NeverInterruptPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    Ok(())
  }
}
