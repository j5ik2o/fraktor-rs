//! Signal delivered before a supervised actor restarts.

use crate::message_and_signals::{BehaviorSignal, Signal};

/// Public signal emitted before the actor is restarted by supervision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreRestart;

impl Signal for PreRestart {}

impl From<PreRestart> for BehaviorSignal {
  fn from(_value: PreRestart) -> Self {
    Self::PreRestart
  }
}
