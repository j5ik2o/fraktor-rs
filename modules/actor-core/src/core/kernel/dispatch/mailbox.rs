//! Mailbox package.
//!
//! This module contains message queue implementations and configurations.

use fraktor_utils_rs::core::{
  collections::queue::{QueueError, SyncFifoQueueShared, SyncQueue, backend::VecDequeBackend, type_keys::FifoKey},
  sync::RuntimeMutex,
};

use crate::core::kernel::actor::{error::SendError, messaging::AnyMessage};

mod backpressure_publisher;
mod base;
/// Bounded mailbox type factory.
mod bounded_mailbox_type;
/// Bounded message queue with configurable overflow strategy.
mod bounded_message_queue;
/// Bounded priority mailbox type factory.
mod bounded_priority_mailbox_type;
/// Bounded priority message queue backed by a binary heap with capacity control.
mod bounded_priority_message_queue;
/// Bounded stable-priority mailbox type factory.
mod bounded_stable_priority_mailbox_type;
/// Bounded stable-priority message queue with capacity control and FIFO ordering within equal
/// priorities.
mod bounded_stable_priority_message_queue;
mod capacity;
/// Opt-in deque capability for message queue implementations.
mod deque_message_queue;
mod envelope;
mod mailbox_cleanup_policy;
mod mailbox_enqueue_outcome;
mod mailbox_instrumentation;
mod mailbox_message;
mod mailbox_offer_future;
mod mailbox_poll_future;
mod mailbox_queue_handles;
mod mailbox_queue_state;
mod mailbox_registry_error;
/// Factory trait for creating message queue instances.
mod mailbox_type;
mod mailboxes;
/// Priority generator for priority-based message queues.
mod message_priority_generator;
/// Pluggable message queue trait.
mod message_queue;
/// Event describing mailbox utilisation metrics.
pub mod metrics_event;
mod overflow_strategy;
mod policy;
mod schedule_hints;
mod schedule_state;
/// Heap entry with sequence number for stable ordering among equal-priority messages.
mod stable_priority_entry;
mod system_queue;
/// Unbounded control-aware mailbox type factory.
mod unbounded_control_aware_mailbox_type;
/// Unbounded control-aware message queue with dual-queue prioritisation.
mod unbounded_control_aware_message_queue;
/// Unbounded deque mailbox type factory.
mod unbounded_deque_mailbox_type;
/// Unbounded deque message queue with O(1) front insertion.
mod unbounded_deque_message_queue;
/// Unbounded mailbox type factory.
mod unbounded_mailbox_type;
/// Unbounded message queue implementation.
mod unbounded_message_queue;
/// Unbounded priority mailbox type factory.
mod unbounded_priority_mailbox_type;
/// Unbounded priority message queue backed by a binary heap.
mod unbounded_priority_message_queue;
/// Unbounded stable-priority mailbox type factory.
mod unbounded_stable_priority_mailbox_type;
/// Unbounded stable-priority message queue with FIFO ordering within equal priorities.
mod unbounded_stable_priority_message_queue;

pub use backpressure_publisher::BackpressurePublisher;
pub use base::Mailbox;
pub use bounded_mailbox_type::BoundedMailboxType;
pub use bounded_message_queue::BoundedMessageQueue;
pub use bounded_priority_mailbox_type::BoundedPriorityMailboxType;
pub use bounded_priority_message_queue::BoundedPriorityMessageQueue;
pub use bounded_stable_priority_mailbox_type::BoundedStablePriorityMailboxType;
pub use bounded_stable_priority_message_queue::BoundedStablePriorityMessageQueue;
pub use capacity::MailboxCapacity;
pub use deque_message_queue::DequeMessageQueue;
pub use envelope::Envelope;
pub use mailbox_cleanup_policy::MailboxCleanupPolicy;
pub use mailbox_enqueue_outcome::EnqueueOutcome;
pub use mailbox_instrumentation::MailboxInstrumentation;
pub(crate) use mailbox_message::MailboxMessage;
pub use mailbox_offer_future::MailboxOfferFuture;
pub use mailbox_poll_future::MailboxPollFuture;
pub(crate) use mailbox_queue_handles::QueueStateHandle;
pub use mailbox_registry_error::MailboxRegistryError;
pub use mailbox_type::MailboxType;
pub use mailboxes::Mailboxes;
pub use message_priority_generator::MessagePriorityGenerator;
pub use message_queue::MessageQueue;
pub use overflow_strategy::MailboxOverflowStrategy;
pub use policy::MailboxPolicy;
pub use schedule_hints::ScheduleHints;
pub(crate) use schedule_state::MailboxScheduleState;
pub(crate) use system_queue::SystemQueue;
pub use unbounded_control_aware_mailbox_type::UnboundedControlAwareMailboxType;
pub use unbounded_control_aware_message_queue::UnboundedControlAwareMessageQueue;
pub use unbounded_deque_mailbox_type::UnboundedDequeMailboxType;
pub use unbounded_deque_message_queue::UnboundedDequeMessageQueue;
pub use unbounded_mailbox_type::UnboundedMailboxType;
pub use unbounded_message_queue::UnboundedMessageQueue;
pub use unbounded_priority_mailbox_type::UnboundedPriorityMailboxType;
pub use unbounded_priority_message_queue::UnboundedPriorityMessageQueue;
pub use unbounded_stable_priority_mailbox_type::UnboundedStablePriorityMailboxType;
pub use unbounded_stable_priority_message_queue::UnboundedStablePriorityMessageQueue;

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
