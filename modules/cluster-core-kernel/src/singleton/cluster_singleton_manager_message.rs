//! Internal hand-over protocol messages exchanged between Cluster Singleton managers.

/// Internal hand-over protocol messages exchanged between managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterSingletonManagerMessage {
  /// Request from the new oldest member to initiate hand-over.
  HandOverToMe,
  /// Confirmation that hand-over has started.
  HandOverInProgress,
  /// Confirmation that hand-over has completed.
  HandOverDone,
  /// Request from the previous oldest member to initiate normal hand-over.
  TakeOverFromMe,
}
