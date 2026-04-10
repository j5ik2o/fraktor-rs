//! Public error type for classic stash overflow.

use core::fmt::{Display, Formatter, Result as FmtResult};

use crate::core::kernel::actor::{ActorContext, STASH_OVERFLOW_REASON, error::ActorError};

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
    ActorError::recoverable(STASH_OVERFLOW_REASON)
  }
}

impl Display for StashOverflowError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{}", STASH_OVERFLOW_REASON)
  }
}
