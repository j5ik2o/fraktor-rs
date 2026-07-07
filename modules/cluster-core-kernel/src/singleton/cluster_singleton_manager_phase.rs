//! Phase of the Cluster Singleton manager state machine.

/// Phase of the Cluster Singleton manager state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterSingletonManagerPhase {
  /// Initial phase before membership is observed.
  Start,
  /// Local member is not the oldest eligible member.
  Younger,
  /// Local member is becoming the oldest member and waiting for hand-over.
  BecomingOldest,
  /// Local member hosts the singleton actor.
  Oldest,
  /// Local member was oldest but is no longer the oldest eligible member.
  WasOldest,
  /// Local member is handing over the singleton actor to the next oldest member.
  HandingOver,
  /// Local member is taking over from a previous oldest member.
  TakeOver,
  /// Local member is stopping the singleton actor.
  Stopping,
  /// Terminal phase after shutdown completes.
  End,
}
