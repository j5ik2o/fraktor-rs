//! Shard coordinator handoff protocol state machine.

use alloc::{collections::BTreeSet, string::String, vec, vec::Vec};

#[cfg(test)]
#[path = "shard_coordinator_handoff_test.rs"]
mod tests;

use super::{
  shard_coordinator_handoff_action::ShardCoordinatorHandoffAction,
  shard_coordinator_handoff_command::ShardCoordinatorHandoffCommand,
  shard_coordinator_handoff_outcome::ShardCoordinatorHandoffOutcome,
};

/// Phase of an in-progress shard handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ShardCoordinatorHandoffPhase {
  /// Waiting for begin-hand-off acknowledgements.
  AwaitingBeginHandOffAcks { shard_id: String, source_region: String, pending_regions: BTreeSet<String> },
  /// Waiting for the source region to stop the shard.
  AwaitingShardStopped { shard_id: String, source_region: String },
}

/// Pure state machine for shard coordinator handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardCoordinatorHandoff {
  phase: Option<ShardCoordinatorHandoffPhase>,
}

impl ShardCoordinatorHandoff {
  /// Creates an idle handoff state machine.
  #[must_use]
  pub const fn new() -> Self {
    Self { phase: None }
  }

  /// Returns whether a handoff is currently in progress.
  #[must_use]
  pub const fn is_active(&self) -> bool {
    self.phase.is_some()
  }

  /// Applies one command and returns outbound actions plus an optional outcome.
  #[must_use = "handoff actions and outcomes must be handled"]
  pub fn apply(
    &mut self,
    command: ShardCoordinatorHandoffCommand,
  ) -> (Vec<ShardCoordinatorHandoffAction>, Option<ShardCoordinatorHandoffOutcome>) {
    match command {
      | ShardCoordinatorHandoffCommand::Start { shard_id, source_region, regions } => {
        self.start(shard_id, source_region, regions)
      },
      | ShardCoordinatorHandoffCommand::BeginHandOffAck { shard_id, region } => {
        self.begin_hand_off_ack(shard_id, &region)
      },
      | ShardCoordinatorHandoffCommand::ShardStopped { shard_id } => self.shard_stopped(shard_id),
      | ShardCoordinatorHandoffCommand::RegionTerminated { region } => self.region_terminated(&region),
      | ShardCoordinatorHandoffCommand::Timeout => self.timeout(),
    }
  }

  fn start(
    &mut self,
    shard_id: String,
    source_region: String,
    regions: BTreeSet<String>,
  ) -> (Vec<ShardCoordinatorHandoffAction>, Option<ShardCoordinatorHandoffOutcome>) {
    self.phase = Some(ShardCoordinatorHandoffPhase::AwaitingBeginHandOffAcks {
      shard_id: shard_id.clone(),
      source_region,
      pending_regions: regions.clone(),
    });

    (vec![ShardCoordinatorHandoffAction::SendBeginHandOff { shard_id, regions }], None)
  }

  fn begin_hand_off_ack(
    &mut self,
    shard_id: String,
    region: &str,
  ) -> (Vec<ShardCoordinatorHandoffAction>, Option<ShardCoordinatorHandoffOutcome>) {
    let Some(ShardCoordinatorHandoffPhase::AwaitingBeginHandOffAcks {
      shard_id: active_shard,
      source_region,
      pending_regions,
    }) = &mut self.phase
    else {
      return (Vec::new(), None);
    };

    if *active_shard != shard_id {
      return (Vec::new(), None);
    }

    pending_regions.remove(region);
    if !pending_regions.is_empty() {
      return (Vec::new(), None);
    }

    let source_region = source_region.clone();
    self.phase = Some(ShardCoordinatorHandoffPhase::AwaitingShardStopped {
      shard_id:      shard_id.clone(),
      source_region: source_region.clone(),
    });

    (vec![ShardCoordinatorHandoffAction::SendHandOff { shard_id, source_region }], None)
  }

  fn shard_stopped(
    &mut self,
    shard_id: String,
  ) -> (Vec<ShardCoordinatorHandoffAction>, Option<ShardCoordinatorHandoffOutcome>) {
    let Some(ShardCoordinatorHandoffPhase::AwaitingShardStopped { shard_id: active_shard, .. }) = &self.phase else {
      return (Vec::new(), None);
    };

    if *active_shard != shard_id {
      return (Vec::new(), None);
    }

    self.phase = None;
    (Vec::new(), Some(ShardCoordinatorHandoffOutcome { shard_id, success: true }))
  }

  fn region_terminated(
    &mut self,
    region: &str,
  ) -> (Vec<ShardCoordinatorHandoffAction>, Option<ShardCoordinatorHandoffOutcome>) {
    match &mut self.phase {
      | Some(ShardCoordinatorHandoffPhase::AwaitingBeginHandOffAcks { shard_id, source_region, pending_regions }) => {
        if pending_regions.remove(region) && pending_regions.is_empty() {
          let shard_id = shard_id.clone();
          let source_region = source_region.clone();
          self.phase = Some(ShardCoordinatorHandoffPhase::AwaitingShardStopped {
            shard_id:      shard_id.clone(),
            source_region: source_region.clone(),
          });
          return (vec![ShardCoordinatorHandoffAction::SendHandOff { shard_id, source_region }], None);
        }
        (Vec::new(), None)
      },
      | Some(ShardCoordinatorHandoffPhase::AwaitingShardStopped { shard_id, source_region })
        if *source_region == region =>
      {
        let shard_id = shard_id.clone();
        self.phase = None;
        (Vec::new(), Some(ShardCoordinatorHandoffOutcome { shard_id, success: true }))
      },
      | _ => (Vec::new(), None),
    }
  }

  fn timeout(&mut self) -> (Vec<ShardCoordinatorHandoffAction>, Option<ShardCoordinatorHandoffOutcome>) {
    let Some(phase) = self.phase.take() else {
      return (Vec::new(), None);
    };

    let shard_id = match phase {
      | ShardCoordinatorHandoffPhase::AwaitingBeginHandOffAcks { shard_id, .. }
      | ShardCoordinatorHandoffPhase::AwaitingShardStopped { shard_id, .. } => shard_id,
    };

    (Vec::new(), Some(ShardCoordinatorHandoffOutcome { shard_id, success: false }))
  }
}

impl Default for ShardCoordinatorHandoff {
  fn default() -> Self {
    Self::new()
  }
}
