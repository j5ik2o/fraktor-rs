//! Internal control messages processed with higher priority than user traffic.

/// Minimal set of system messages required for the core runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemMessage {
  /// Requests the mailbox to suspend user message delivery.
  Suspend,
  /// Requests the mailbox to resume user message delivery.
  Resume,
  /// Signals that the actor should stop execution gracefully.
  Stop,
  /// Internal notification used to wake the dispatcher after new work arrives.
  Wake,
}
