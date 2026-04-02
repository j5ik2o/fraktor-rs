//! Public error type for classic stash overflow.

use core::fmt;

use crate::core::kernel::actor::{ActorContext, error::ActorError};

/// Indicates that a classic actor stash exceeded its configured capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StashOverflowError;

impl TryFrom<ActorError> for StashOverflowError {
  type Error = ActorError;

  fn try_from(error: ActorError) -> Result<Self, Self::Error> {
    if ActorContext::is_stash_overflow_error(&error) { Ok(Self) } else { Err(error) }
  }
}

impl From<StashOverflowError> for ActorError {
  fn from(_: StashOverflowError) -> Self {
    ActorError::recoverable(crate::core::kernel::actor::STASH_OVERFLOW_REASON)
  }
}

impl fmt::Display for StashOverflowError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", crate::core::kernel::actor::STASH_OVERFLOW_REASON)
  }
}
