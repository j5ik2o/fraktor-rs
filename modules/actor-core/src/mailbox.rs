//! Mailbox package.
//!
//! This module contains message queue implementations and configurations.

use fraktor_utils_core_rs::core::collections::queue::{QueueError, SyncFifoQueueShared, backend::VecDequeBackend};

use crate::{RuntimeToolbox, error::SendError, messaging::AnyMessageGeneric};

mod backpressure_publisher;
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
mod state_engine;
mod system_queue;

pub use backpressure_publisher::BackpressurePublisherGeneric;
pub use base::{Mailbox, MailboxGeneric};
pub use capacity::MailboxCapacity;
pub use mailbox_enqueue_outcome::EnqueueOutcome;
pub use mailbox_instrumentation::{MailboxInstrumentation, MailboxInstrumentationGeneric};
pub use mailbox_message::MailboxMessage;
pub use mailbox_offer_future::{MailboxOfferFuture, MailboxOfferFutureGeneric};
pub use mailbox_poll_future::{MailboxPollFuture, MailboxPollFutureGeneric};
pub use mailbox_queue_handles::QueueHandles;
pub use mailbox_queue_offer_future::QueueOfferFuture;
pub use mailbox_queue_poll_future::QueuePollFuture;
pub use mailbox_queue_state::QueueState;
pub use metrics_event::{MailboxMetricsEvent, MailboxPressureEvent};
pub use overflow_strategy::MailboxOverflowStrategy;
pub use policy::MailboxPolicy;
pub use state_engine::{MailboxStateEngine, ScheduleHints};
pub use system_queue::SystemQueue;

#[cfg(test)]
mod tests;

pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>>;

pub(crate) fn map_user_queue_error<TB: RuntimeToolbox>(error: QueueError<AnyMessageGeneric<TB>>) -> SendError<TB> {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(item),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(item),
    | QueueError::TimedOut(item) => SendError::timeout(item),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}
