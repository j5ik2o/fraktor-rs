//! Signal delivered after an actor stops.

use crate::core::typed::message_and_signals::{BehaviorSignal, Signal};

/// Public signal emitted when the actor is stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostStop;

impl Signal for PostStop {}

impl From<PostStop> for BehaviorSignal {
  fn from(_value: PostStop) -> Self {
    Self::PostStop
  }
}
