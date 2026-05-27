//! Result of snapshot plugin message handling.

/// Indicates whether a snapshot plugin consumed a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginMessageHandling {
  /// The plugin consumed the message.
  Handled,
  /// The plugin did not consume the message.
  Unhandled,
}
