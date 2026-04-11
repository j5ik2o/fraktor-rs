//! Factory contract for [`MessageInvokerShared`](super::MessageInvokerShared).

use alloc::boxed::Box;

use super::{MessageInvoker, MessageInvokerShared};

/// Materializes [`MessageInvokerShared`] instances.
pub trait MessageInvokerSharedFactory: Send + Sync {
  /// Creates a shared message-invoker wrapper.
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared;
}
