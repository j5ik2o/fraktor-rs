//! Priority mailbox managing system and user message queues.

use cellactor_utils_core_rs::{
  collections::queue::{QueueError, backend::VecRingBackend},
  sync::sync_mutex_like::SpinSyncMutex,
};

use crate::{any_message::AnyOwnedMessage, send_error::SendError, system_message::SystemMessage};

mod enqueue_outcome;
mod mailbox_impl;
mod mailbox_message;
mod mailbox_offer_future;
mod mailbox_poll_future;
mod queue_handles;
mod queue_offer_future;
mod queue_poll_future;
mod queue_state;
#[cfg(test)]
mod tests;

pub use enqueue_outcome::EnqueueOutcome;
pub use mailbox_impl::Mailbox;
pub use mailbox_message::MailboxMessage;
pub use mailbox_offer_future::MailboxOfferFuture;
pub use mailbox_poll_future::MailboxPollFuture;

type QueueMutex<T> = SpinSyncMutex<VecRingBackend<T>>;

fn map_user_queue_error(error: QueueError<AnyOwnedMessage>) -> SendError {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(item),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(item),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}

fn map_system_queue_error(error: QueueError<SystemMessage>) -> SendError {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(AnyOwnedMessage::new(item)),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(AnyOwnedMessage::new(item)),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}
