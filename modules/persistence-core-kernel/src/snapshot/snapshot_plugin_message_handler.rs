//! Message hook for advanced snapshot plugins.

use fraktor_actor_core_kernel_rs::actor::{ActorContext, error::ActorError, messaging::AnyMessageView};

use crate::snapshot::PluginMessageHandling;

/// Handles snapshot plugin specific messages.
pub trait SnapshotPluginMessageHandler: Send {
  /// Handles a snapshot plugin message or observes a snapshot response.
  ///
  /// # Errors
  ///
  /// Returns an actor error when plugin-specific message processing fails.
  fn handle_snapshot_plugin_message(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: AnyMessageView<'_>,
  ) -> Result<PluginMessageHandling, ActorError>;
}
