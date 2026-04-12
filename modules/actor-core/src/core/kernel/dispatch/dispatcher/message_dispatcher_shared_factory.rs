//! Factory contract for [`MessageDispatcherShared`](super::MessageDispatcherShared).

use alloc::boxed::Box;

use super::{MessageDispatcher, MessageDispatcherShared};

/// Materializes [`MessageDispatcherShared`] instances.
pub trait MessageDispatcherSharedFactory: Send + Sync {
  /// Creates a shared dispatcher wrapper.
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared;
}
