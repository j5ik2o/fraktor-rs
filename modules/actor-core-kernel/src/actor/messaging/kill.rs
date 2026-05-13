//! Public classic auto-receive message that requests fatal actor termination.

use crate::actor::messaging::system_message::SystemMessage;

/// Public classic auto-receive message that requests fatal actor termination.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kill;

impl From<Kill> for SystemMessage {
  fn from(_value: Kill) -> Self {
    Self::Kill
  }
}
