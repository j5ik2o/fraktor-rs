//! Factory contract for bounded stable-priority mailbox state.

use super::{
  bounded_stable_priority_message_queue_state::BoundedStablePriorityMessageQueueState,
  bounded_stable_priority_message_queue_state_shared::BoundedStablePriorityMessageQueueStateShared,
};

/// Materializes shared state for bounded stable-priority message queues.
pub trait BoundedStablePriorityMessageQueueStateSharedFactory: Send + Sync {
  /// Wraps the queue state in the selected shared-lock family.
  fn create_bounded_stable_priority_message_queue_state_shared(
    &self,
    state: BoundedStablePriorityMessageQueueState,
  ) -> BoundedStablePriorityMessageQueueStateShared;
}
