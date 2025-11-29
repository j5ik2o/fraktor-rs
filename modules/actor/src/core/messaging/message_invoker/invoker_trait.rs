//! Trait for dispatching messages from the mailbox to actors.

extern crate alloc;

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::{
  error::ActorError,
  messaging::{AnyMessageGeneric, SystemMessage},
};

/// Shared handle for invoking a message invoker under external synchronization.
pub type MessageInvokerShared<TB> = ArcShared<ToolboxMutex<Box<dyn MessageInvoker<TB>>, TB>>;

/// Dispatches user and system messages to actor handlers.
pub trait MessageInvoker<TB: RuntimeToolbox + 'static = NoStdToolbox>: Send + Sync {
  /// Processes user messages.
  ///
  /// # Errors
  ///
  /// Returns an error if message processing fails.
  fn invoke_user_message(&mut self, message: AnyMessageGeneric<TB>) -> Result<(), ActorError>;

  /// Processes system messages.
  ///
  /// # Errors
  ///
  /// Returns an error if system message processing fails.
  fn invoke_system_message(&mut self, message: SystemMessage) -> Result<(), ActorError>;
}
