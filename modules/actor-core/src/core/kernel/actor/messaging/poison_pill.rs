//! Public classic auto-receive message that requests graceful actor termination.

use crate::core::kernel::actor::messaging::system_message::SystemMessage;

/// Public classic auto-receive message that requests graceful actor termination.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoisonPill;

impl From<PoisonPill> for SystemMessage {
  fn from(_value: PoisonPill) -> Self {
    Self::PoisonPill
  }
}
