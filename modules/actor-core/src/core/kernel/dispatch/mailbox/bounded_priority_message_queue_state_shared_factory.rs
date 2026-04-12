//! Factory contract for bounded-priority mailbox state.

use super::bounded_priority_message_queue_state_shared::{
  BoundedPriorityMessageQueueState, BoundedPriorityMessageQueueStateShared,
};

/// Materializes shared state for bounded-priority message queues.
pub trait BoundedPriorityMessageQueueStateSharedFactory: Send + Sync {
  /// Wraps the queue state in the selected shared-lock family.
  fn create_bounded_priority_message_queue_state_shared(
    &self,
    state: BoundedPriorityMessageQueueState,
  ) -> BoundedPriorityMessageQueueStateShared;
}
