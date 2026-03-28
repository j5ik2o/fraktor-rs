//! Scheduler command representations.

use core::fmt;

use fraktor_utils_rs::core::sync::ArcShared;

use super::{dispatcher_sender_shared::DispatcherSenderShared, runnable::SchedulerRunnable};
use crate::core::kernel::{actor::actor_ref::ActorRef, messaging::AnyMessage};

/// Commands executed when scheduled timers fire.
#[derive(Clone)]
pub enum SchedulerCommand {
  /// Placeholder used while the runner integration is under construction.
  Noop,
  /// Sends a message to the target actor through the scheduler pipeline.
  SendMessage {
    /// Target actor reference receiving the message.
    receiver:   ActorRef,
    /// Message payload to be enqueued.
    message:    AnyMessage,
    /// Dispatcher used to enqueue the message (if explicitly provided).
    dispatcher: Option<DispatcherSenderShared>,
    /// Logical sender recorded for diagnostics.
    sender:     Option<ActorRef>,
  },
  /// Runs a closure-style task when the timer fires.
  RunRunnable {
    /// Runnable invoked once the timer expires.
    runnable:   ArcShared<dyn SchedulerRunnable>,
    /// Dispatcher requested for runnable execution, when available.
    dispatcher: Option<DispatcherSenderShared>,
  },
}

impl fmt::Debug for SchedulerCommand {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Noop => write!(f, "SchedulerCommand::Noop"),
      | Self::SendMessage { receiver, .. } => {
        f.debug_struct("SchedulerCommand::SendMessage").field("receiver", receiver).finish()
      },
      | Self::RunRunnable { .. } => write!(f, "SchedulerCommand::RunRunnable"),
    }
  }
}
