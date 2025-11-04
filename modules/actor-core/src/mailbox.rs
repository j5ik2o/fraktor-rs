//! Mailbox package.
//!
//! This module contains message queue implementations and configurations.

use cellactor_utils_core_rs::collections::queue::{QueueError, backend::VecRingBackend};

use crate::{
  RuntimeToolbox, ToolboxMutex,
  error::SendError,
  messaging::{AnyMessage, SystemMessage},
};

mod base;
mod capacity;
mod mailbox_enqueue_outcome;
mod mailbox_instrumentation;
mod mailbox_message;
mod mailbox_offer_future;
mod mailbox_poll_future;
mod mailbox_queue_handles;
mod mailbox_queue_offer_future;
mod mailbox_queue_poll_future;
mod mailbox_queue_state;
mod metrics_event;
mod overflow_strategy;
mod policy;

pub use base::Mailbox;
pub use capacity::MailboxCapacity;
pub use mailbox_enqueue_outcome::EnqueueOutcome;
pub use mailbox_instrumentation::MailboxInstrumentation;
pub use mailbox_message::MailboxMessage;
pub use mailbox_offer_future::MailboxOfferFuture;
pub use mailbox_poll_future::MailboxPollFuture;
pub use mailbox_queue_handles::QueueHandles;
pub use mailbox_queue_offer_future::QueueOfferFuture;
pub use mailbox_queue_poll_future::QueuePollFuture;
pub use mailbox_queue_state::QueueState;
pub use metrics_event::MailboxMetricsEvent;
pub use overflow_strategy::MailboxOverflowStrategy;
pub use policy::MailboxPolicy;

#[cfg(test)]
mod tests;

pub(crate) type QueueMutex<T, TB> = ToolboxMutex<VecRingBackend<T>, TB>;

pub(crate) fn map_user_queue_error<TB: RuntimeToolbox>(error: QueueError<AnyMessage<TB>>) -> SendError<TB> {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(item),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(item),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}

pub(crate) fn map_system_queue_error<TB: RuntimeToolbox>(error: QueueError<SystemMessage>) -> SendError<TB> {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(AnyMessage::new(item)),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(AnyMessage::new(item)),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}
