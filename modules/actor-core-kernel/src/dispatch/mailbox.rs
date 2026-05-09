//! Mailbox package.
//!
//! This module contains message queue implementations and configurations.

use fraktor_utils_core_rs::collections::queue::{QueueError, SyncQueueShared, backend::VecDequeBackend};

use crate::actor::{error::SendError, messaging::AnyMessage};

mod backpressure_publisher;
mod base;
/// Bounded control-aware mailbox type factory.
mod bounded_control_aware_mailbox_type;
/// Bounded control-aware message queue with dual-queue prioritisation and capacity enforcement.
mod bounded_control_aware_message_queue;
/// Bounded deque mailbox type factory.
mod bounded_deque_mailbox_type;
/// Bounded deque message queue with capacity enforcement and O(1) front insertion.
mod bounded_deque_message_queue;
/// Bounded mailbox type factory.
mod bounded_mailbox_type;
/// Bounded message queue with configurable overflow strategy.
mod bounded_message_queue;
/// Bounded priority mailbox type factory.
mod bounded_priority_mailbox_type;
/// Bounded priority message queue backed by a binary heap with capacity control.
mod bounded_priority_message_queue;
mod bounded_priority_message_queue_state;
mod bounded_priority_message_queue_state_shared;
/// Bounded stable-priority mailbox type factory.
mod bounded_stable_priority_mailbox_type;
/// Bounded stable-priority message queue with capacity control and FIFO ordering within equal
/// priorities.
mod bounded_stable_priority_message_queue;
mod bounded_stable_priority_message_queue_state;
mod bounded_stable_priority_message_queue_state_shared;
mod capacity;
/// Opt-in deque capability for message queue implementations.
mod deque_message_queue;
mod drop_oldest_error;
mod drop_oldest_outcome;
mod enqueue_error;
mod enqueue_outcome;
mod envelope;
mod lock_free_mpsc_queue;
mod mailbox_cleanup_policy;
/// Monotonic clock callback type for throughput deadline enforcement.
mod mailbox_clock;
/// High-level factory trait registered with the actor-system builder.
mod mailbox_factory;
mod mailbox_instrumentation;
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
mod unbounded_priority_message_queue_state;
mod unbounded_priority_message_queue_state_shared;
/// Unbounded stable-priority mailbox type factory.
mod unbounded_stable_priority_mailbox_type;
/// Unbounded stable-priority message queue with FIFO ordering within equal priorities.
mod unbounded_stable_priority_message_queue;

pub use backpressure_publisher::BackpressurePublisher;
pub use base::Mailbox;
pub use bounded_control_aware_mailbox_type::BoundedControlAwareMailboxType;
pub use bounded_control_aware_message_queue::BoundedControlAwareMessageQueue;
pub use bounded_deque_mailbox_type::BoundedDequeMailboxType;
pub use bounded_deque_message_queue::BoundedDequeMessageQueue;
pub use bounded_mailbox_type::BoundedMailboxType;
pub use bounded_message_queue::BoundedMessageQueue;
pub use bounded_priority_mailbox_type::BoundedPriorityMailboxType;
pub use bounded_priority_message_queue::BoundedPriorityMessageQueue;
pub use bounded_priority_message_queue_state::BoundedPriorityMessageQueueState;
pub use bounded_priority_message_queue_state_shared::BoundedPriorityMessageQueueStateShared;
pub use bounded_stable_priority_mailbox_type::BoundedStablePriorityMailboxType;
pub use bounded_stable_priority_message_queue::BoundedStablePriorityMessageQueue;
pub use bounded_stable_priority_message_queue_state::BoundedStablePriorityMessageQueueState;
pub use bounded_stable_priority_message_queue_state_shared::BoundedStablePriorityMessageQueueStateShared;
pub use capacity::MailboxCapacity;
pub use deque_message_queue::DequeMessageQueue;
pub use enqueue_error::EnqueueError;
pub use enqueue_outcome::EnqueueOutcome;
pub use envelope::Envelope;
pub use mailbox_cleanup_policy::MailboxCleanupPolicy;
pub use mailbox_clock::MailboxClock;
pub use mailbox_factory::MailboxFactory;
pub use mailbox_instrumentation::MailboxInstrumentation;
pub use mailbox_poll_future::MailboxPollFuture;
pub(crate) use mailbox_queue_handles::QueueStateHandle;
pub use mailbox_registry_error::MailboxRegistryError;
pub use mailbox_type::MailboxType;
pub use mailboxes::Mailboxes;
pub(crate) use mailboxes::{create_message_queue_from_config, select_mailbox_type_from_config};
pub use message_priority_generator::MessagePriorityGenerator;
pub use message_queue::MessageQueue;
pub use overflow_strategy::MailboxOverflowStrategy;
pub use policy::MailboxPolicy;
pub use schedule_hints::ScheduleHints;
pub(crate) use schedule_state::{CloseRequestOutcome, MailboxScheduleState, RunFinishOutcome};
pub(crate) use system_queue::SystemQueue;
pub use unbounded_control_aware_mailbox_type::UnboundedControlAwareMailboxType;
pub use unbounded_control_aware_message_queue::UnboundedControlAwareMessageQueue;
pub use unbounded_deque_mailbox_type::UnboundedDequeMailboxType;
pub use unbounded_deque_message_queue::UnboundedDequeMessageQueue;
pub use unbounded_mailbox_type::UnboundedMailboxType;
pub use unbounded_message_queue::UnboundedMessageQueue;
pub use unbounded_priority_mailbox_type::UnboundedPriorityMailboxType;
pub use unbounded_priority_message_queue::UnboundedPriorityMessageQueue;
pub use unbounded_priority_message_queue_state::UnboundedPriorityMessageQueueState;
pub use unbounded_priority_message_queue_state_shared::UnboundedPriorityMessageQueueStateShared;
pub use unbounded_stable_priority_mailbox_type::UnboundedStablePriorityMailboxType;
pub use unbounded_stable_priority_message_queue::UnboundedStablePriorityMessageQueue;

#[cfg(test)]
mod tests;

pub(crate) type UserQueueShared<T> = SyncQueueShared<T, VecDequeBackend<T>>;

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

pub(crate) fn map_user_envelope_queue_error(error: QueueError<Envelope>) -> SendError {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(item.into_payload()),
    | QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(item.into_payload()),
    | QueueError::TimedOut(item) => SendError::timeout(item.into_payload()),
    | QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}
