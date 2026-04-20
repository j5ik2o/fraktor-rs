//! Signal delivered after a supervised actor restarts.

use crate::core::typed::message_and_signals::{BehaviorSignal, Signal};

/// Public signal emitted after the actor has been restarted by supervision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostRestart;

impl Signal for PostRestart {}

impl From<PostRestart> for BehaviorSignal {
  fn from(_value: PostRestart) -> Self {
    Self::PostRestart
  }
}
