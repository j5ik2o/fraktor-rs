//! Message hook for advanced journal plugins.

use fraktor_actor_core_kernel_rs::actor::{ActorContext, error::ActorError, messaging::AnyMessageView};

use crate::PluginMessageHandling;

/// Handles journal plugin specific messages.
pub trait JournalPluginMessageHandler: Send {
  /// Handles a message not consumed by the built-in journal protocol.
  ///
  /// # Errors
  ///
  /// Returns an actor error when plugin-specific message processing fails.
  fn handle_journal_plugin_message(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: AnyMessageView<'_>,
  ) -> Result<PluginMessageHandling, ActorError>;
}
