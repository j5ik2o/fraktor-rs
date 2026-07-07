//! Effect requested by the Cluster Singleton manager state machine.

use alloc::string::String;

use super::singleton_stuck_phase::SingletonStuckPhase;

/// Effect requested by the manager state machine for the runtime driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterSingletonManagerEffect {
  /// Start the singleton actor on the local node.
  StartSingleton,
  /// Stop the singleton actor on the local node.
  StopSingleton,
  /// Send `HandOverToMe` to the target authority.
  SendHandOverToMe {
    /// Target node authority.
    target_authority: String,
  },
  /// Send `TakeOverFromMe` to the target authority.
  SendTakeOverFromMe {
    /// Target node authority.
    target_authority: String,
  },
  /// Send `HandOverDone` to the hand-over requester.
  SendHandOverDone,
  /// Publish a stuck hand-over observation event.
  PublishHandOverStuck {
    /// Stuck phase to observe.
    phase: SingletonStuckPhase,
  },
  /// Schedule the next hand-over retry tick.
  ScheduleHandOverRetry,
}
