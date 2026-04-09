//! Scheduler command representations.

use alloc::boxed::Box;
use core::fmt;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::runnable::SchedulerRunnable;
use crate::core::kernel::actor::{actor_ref::ActorRef, messaging::AnyMessage};

/// Commands executed when scheduled timers fire.
#[derive(Clone)]
pub enum SchedulerCommand {
  /// Placeholder used while the runner integration is under construction.
  Noop,
  /// Sends a message to the target actor through the scheduler pipeline.
  SendMessage(Box<(ActorRef, AnyMessage, Option<ActorRef>)>),
  /// Runs a closure-style task when the timer fires.
  RunRunnable {
    /// Runnable invoked once the timer expires.
    runnable: ArcShared<dyn SchedulerRunnable>,
  },
}

impl fmt::Debug for SchedulerCommand {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Noop => write!(f, "SchedulerCommand::Noop"),
      | Self::SendMessage(command) => {
        f.debug_struct("SchedulerCommand::SendMessage").field("receiver", &command.0).finish()
      },
      | Self::RunRunnable { .. } => write!(f, "SchedulerCommand::RunRunnable"),
    }
  }
}

impl SchedulerCommand {
  /// Creates a scheduler command that delivers a message to an actor reference.
  #[must_use]
  pub fn send_message(receiver: ActorRef, message: AnyMessage, sender: Option<ActorRef>) -> Self {
    Self::SendMessage(Box::new((receiver, message, sender)))
  }

  #[must_use]
  pub(crate) fn send_message_parts(&self) -> Option<(&ActorRef, &AnyMessage, Option<&ActorRef>)> {
    match self {
      | Self::SendMessage(command) => Some((&command.0, &command.1, command.2.as_ref())),
      | _ => None,
    }
  }
}
