//! Factory contract for unbounded-priority mailbox state.

use super::{
  unbounded_priority_message_queue_state::UnboundedPriorityMessageQueueState,
  unbounded_priority_message_queue_state_shared::UnboundedPriorityMessageQueueStateShared,
};

/// Materializes shared state for unbounded-priority message queues.
pub trait UnboundedPriorityMessageQueueStateSharedFactory: Send + Sync {
  /// Wraps the queue state in the selected shared-lock family.
  fn create_unbounded_priority_message_queue_state_shared(
    &self,
    state: UnboundedPriorityMessageQueueState,
  ) -> UnboundedPriorityMessageQueueStateShared;
}
