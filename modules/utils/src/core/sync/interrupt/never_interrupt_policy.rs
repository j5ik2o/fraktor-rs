use super::InterruptContextPolicy;
use crate::core::sync::SharedError;

#[cfg(test)]
mod tests;

/// Policy that never reports an active interrupt context.
#[allow(dead_code)]
pub(crate) struct NeverInterruptPolicy;

impl InterruptContextPolicy for NeverInterruptPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    Ok(())
  }
}
