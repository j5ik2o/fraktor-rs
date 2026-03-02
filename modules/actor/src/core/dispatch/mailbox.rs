//! Mailbox package.
//!
//! This module contains message queue implementations and configurations.

use fraktor_utils_rs::core::{
  collections::queue::{QueueError, SyncFifoQueueShared, SyncQueue, backend::VecDequeBackend, type_keys::FifoKey},
  runtime_toolbox::RuntimeMutex,
};

use crate::core::{error::SendError, messaging::AnyMessage};

mod backpressure_publisher;
mod base;
mod capacity;
mod mailbox_enqueue_outcome;
mod mailbox_instrumentation;
mod mailbox_message;
mod mailbox_offer_future;
mod mailbox_poll_future;
mod mailbox_queue_handles;
mod mailbox_queue_state;
mod mailbox_registry_error;
mod mailboxes;
/// Event describing mailbox utilisation metrics.
pub mod metrics_event;
mod overflow_strategy;
mod policy;
mod schedule_hints;
mod schedule_state;
mod system_queue;

pub use backpressure_publisher::BackpressurePublisher;
pub use base::Mailbox;
pub use capacity::MailboxCapacity;
pub use mailbox_enqueue_outcome::EnqueueOutcome;
pub use mailbox_instrumentation::MailboxInstrumentation;
pub(crate) use mailbox_message::MailboxMessage;
pub use mailbox_offer_future::MailboxOfferFuture;
pub use mailbox_poll_future::MailboxPollFuture;
pub(crate) use mailbox_queue_handles::QueueStateHandle;
pub use mailbox_registry_error::MailboxRegistryError;
pub use mailboxes::Mailboxes;
pub use overflow_strategy::MailboxOverflowStrategy;
pub use policy::MailboxPolicy;
pub use schedule_hints::ScheduleHints;
pub(crate) use schedule_state::MailboxScheduleState;
pub(crate) use system_queue::SystemQueue;

#[cfg(test)]
mod tests;

pub(crate) type UserQueueShared<T> =
  SyncFifoQueueShared<T, VecDequeBackend<T>, RuntimeMutex<SyncQueue<T, FifoKey, VecDequeBackend<T>>>>;

pub(crate) fn map_user_queue_error(error: QueueError<AnyMessage>) -> SendError {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(item),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(item),
    | QueueError::TimedOut(item) => SendError::timeout(item),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}
