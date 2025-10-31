use super::InterruptContextPolicy;
use crate::sync::SharedError;

/// Policy that consults platform-specific interrupt state before allowing blocking operations.
pub struct CriticalSectionInterruptPolicy;

impl InterruptContextPolicy for CriticalSectionInterruptPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    #[cfg(feature = "interrupt-cortex-m")]
    {
      use cortex_m::peripheral::{SCB, scb::VectActive};

      if !matches!(SCB::vect_active(), VectActive::ThreadMode) {
        return Err(SharedError::InterruptContext);
      }
    }

    Ok(())
  }
}
