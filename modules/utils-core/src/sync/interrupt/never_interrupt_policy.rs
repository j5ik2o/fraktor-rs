use super::InterruptContextPolicy;
use crate::sync::SharedError;

#[cfg(test)]
mod tests;

/// Policy that never reports an active interrupt context.
pub struct NeverInterruptPolicy;

impl InterruptContextPolicy for NeverInterruptPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    Ok(())
  }
}
