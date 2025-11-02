//! Priority mailbox managing system and user message queues.

use cellactor_utils_core_rs::collections::queue::{QueueError, backend::VecRingBackend};

use crate::{AnyMessage, RuntimeToolbox, SendError, SystemMessage};

mod mailbox_enqueue_outcome;
mod mailbox_impl;
mod mailbox_instrumentation;
mod mailbox_message;
mod mailbox_offer_future;
mod mailbox_poll_future;
mod mailbox_queue_handles;
mod mailbox_queue_offer_future;
mod mailbox_queue_poll_future;
mod mailbox_queue_state;

#[allow(unused_imports)]
pub use mailbox_enqueue_outcome::EnqueueOutcome;
pub use mailbox_impl::Mailbox;
#[allow(unused_imports)]
pub use mailbox_instrumentation::MailboxInstrumentation;
#[allow(unused_imports)]
pub use mailbox_message::MailboxMessage;
#[allow(unused_imports)]
pub use mailbox_offer_future::MailboxOfferFuture;
#[allow(unused_imports)]
pub use mailbox_poll_future::MailboxPollFuture;

#[cfg(test)]
mod tests;

type QueueMutex<T, TB> = crate::ToolboxMutex<VecRingBackend<T>, TB>;

fn map_user_queue_error<TB: RuntimeToolbox>(error: QueueError<AnyMessage<TB>>) -> SendError<TB> {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(item),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(item),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}

fn map_system_queue_error<TB: RuntimeToolbox>(error: QueueError<SystemMessage>) -> SendError<TB> {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(AnyMessage::new(item)),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(AnyMessage::new(item)),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}
