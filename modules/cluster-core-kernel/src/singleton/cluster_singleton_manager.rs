//! Cluster Singleton manager runtime state machine.

#[cfg(test)]
#[path = "cluster_singleton_manager_test.rs"]
mod tests;

use alloc::{string::String, vec, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use super::{ClusterSingletonManagerConfig, SingletonStuckPhase};
use crate::membership::{NodeRecord, age_ordered, member_age_order, oldest_member};

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
  /// Publish a stuck hand-over observation event.
  PublishHandOverStuck {
    /// Stuck phase to observe.
    phase: SingletonStuckPhase,
  },
  /// Schedule the next hand-over retry tick.
  ScheduleHandOverRetry,
}

/// Outcome produced by applying manager input to the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSingletonManagerOutcome {
  /// New manager phase after the transition.
  pub phase:   ClusterSingletonManagerPhase,
  /// Effects for the runtime driver to execute.
  pub effects: Vec<ClusterSingletonManagerEffect>,
}

impl ClusterSingletonManagerOutcome {
  fn with_phase(phase: ClusterSingletonManagerPhase) -> Self {
    Self { phase, effects: Vec::new() }
  }

  fn with_effect(phase: ClusterSingletonManagerPhase, effect: ClusterSingletonManagerEffect) -> Self {
    Self { phase, effects: vec![effect] }
  }

  fn with_effects(phase: ClusterSingletonManagerPhase, effects: Vec<ClusterSingletonManagerEffect>) -> Self {
    Self { phase, effects }
  }
}

/// Pure state machine for Cluster Singleton manager runtime behavior.
#[derive(Debug, Clone)]
pub struct ClusterSingletonManager {
  config:                ClusterSingletonManagerConfig,
  phase:                 ClusterSingletonManagerPhase,
  local_authority:       String,
  oldest_members:        Vec<String>,
  previous_oldest:       Option<String>,
  hand_over_retry_count: u32,
  singleton_running:     bool,
  next_retry_at:         Option<TimerInstant>,
}

impl ClusterSingletonManager {
  /// Creates a manager in the initial start phase.
  #[must_use]
  pub fn new(config: ClusterSingletonManagerConfig, local_authority: impl Into<String>) -> Self {
    Self {
      config,
      phase: ClusterSingletonManagerPhase::Start,
      local_authority: local_authority.into(),
      oldest_members: Vec::new(),
      previous_oldest: None,
      hand_over_retry_count: 0,
      singleton_running: false,
      next_retry_at: None,
    }
  }

  /// Returns the current manager phase.
  #[must_use]
  pub const fn phase(&self) -> ClusterSingletonManagerPhase {
    self.phase
  }

  /// Returns true when the singleton actor is considered running locally.
  #[must_use]
  pub const fn singleton_running(&self) -> bool {
    self.singleton_running
  }

  /// Returns the configured manager settings.
  #[must_use]
  pub const fn config(&self) -> &ClusterSingletonManagerConfig {
    &self.config
  }

  /// Applies a membership snapshot and advances the manager state machine.
  #[must_use]
  pub fn apply_topology(&mut self, members: &[NodeRecord], now: TimerInstant) -> ClusterSingletonManagerOutcome {
    let eligible = eligible_members(members, self.config.role());
    let ordered = age_ordered(&eligible);
    self.oldest_members = ordered.iter().map(|record| record.authority.clone()).collect();

    let local_is_oldest = ordered.first().is_some_and(|record| record.authority == self.local_authority);
    let previous_oldest = self.previous_oldest.clone();

    match self.phase {
      | ClusterSingletonManagerPhase::Start => {
        if ordered.is_empty() {
          return ClusterSingletonManagerOutcome::with_phase(self.phase);
        }
        if local_is_oldest {
          self.phase = ClusterSingletonManagerPhase::Oldest;
          self.singleton_running = true;
          return ClusterSingletonManagerOutcome::with_effect(
            self.phase,
            ClusterSingletonManagerEffect::StartSingleton,
          );
        }
        self.phase = ClusterSingletonManagerPhase::Younger;
        ClusterSingletonManagerOutcome::with_phase(self.phase)
      },
      | ClusterSingletonManagerPhase::Younger => {
        if !local_is_oldest {
          return ClusterSingletonManagerOutcome::with_phase(self.phase);
        }
        if let Some(previous) = previous_oldest.filter(|authority| authority != &self.local_authority)
          && still_present(&eligible, previous.as_str())
        {
          self.phase = ClusterSingletonManagerPhase::BecomingOldest;
          self.hand_over_retry_count = 0;
          self.next_retry_at = Some(schedule_after(now, self.config.hand_over_retry_interval()));
          return ClusterSingletonManagerOutcome::with_effects(self.phase, vec![
            ClusterSingletonManagerEffect::SendHandOverToMe { target_authority: previous },
            ClusterSingletonManagerEffect::ScheduleHandOverRetry,
          ]);
        }
        self.phase = ClusterSingletonManagerPhase::Oldest;
        self.singleton_running = true;
        ClusterSingletonManagerOutcome::with_effect(self.phase, ClusterSingletonManagerEffect::StartSingleton)
      },
      | ClusterSingletonManagerPhase::Oldest => {
        if local_is_oldest {
          return ClusterSingletonManagerOutcome::with_phase(self.phase);
        }
        self.phase = ClusterSingletonManagerPhase::WasOldest;
        self.previous_oldest = Some(self.local_authority.clone());
        ClusterSingletonManagerOutcome::with_effect(self.phase, ClusterSingletonManagerEffect::StopSingleton)
      },
      | ClusterSingletonManagerPhase::WasOldest
      | ClusterSingletonManagerPhase::HandingOver
      | ClusterSingletonManagerPhase::TakeOver
      | ClusterSingletonManagerPhase::BecomingOldest
      | ClusterSingletonManagerPhase::Stopping
      | ClusterSingletonManagerPhase::End => ClusterSingletonManagerOutcome::with_phase(self.phase),
    }
  }

  /// Handles an internal hand-over protocol message.
  #[must_use]
  pub fn handle_message(&mut self, message: ClusterSingletonManagerMessage) -> ClusterSingletonManagerOutcome {
    match (self.phase, message) {
      | (ClusterSingletonManagerPhase::Oldest, ClusterSingletonManagerMessage::HandOverToMe) => {
        self.phase = ClusterSingletonManagerPhase::HandingOver;
        self.singleton_running = false;
        ClusterSingletonManagerOutcome::with_effect(self.phase, ClusterSingletonManagerEffect::StopSingleton)
      },
      | (ClusterSingletonManagerPhase::HandingOver, ClusterSingletonManagerMessage::HandOverInProgress) => {
        ClusterSingletonManagerOutcome::with_phase(self.phase)
      },
      | (ClusterSingletonManagerPhase::HandingOver, ClusterSingletonManagerMessage::HandOverDone) => {
        self.phase = ClusterSingletonManagerPhase::End;
        ClusterSingletonManagerOutcome::with_phase(self.phase)
      },
      | (ClusterSingletonManagerPhase::BecomingOldest, ClusterSingletonManagerMessage::HandOverInProgress) => {
        ClusterSingletonManagerOutcome::with_phase(self.phase)
      },
      | (ClusterSingletonManagerPhase::BecomingOldest, ClusterSingletonManagerMessage::HandOverDone) => {
        self.phase = ClusterSingletonManagerPhase::Oldest;
        self.singleton_running = true;
        self.hand_over_retry_count = 0;
        self.next_retry_at = None;
        ClusterSingletonManagerOutcome::with_effect(self.phase, ClusterSingletonManagerEffect::StartSingleton)
      },
      | _ => ClusterSingletonManagerOutcome::with_phase(self.phase),
    }
  }

  /// Polls retry timers for hand-over progress.
  #[must_use]
  pub fn poll(&mut self, now: TimerInstant) -> ClusterSingletonManagerOutcome {
    if self.phase != ClusterSingletonManagerPhase::BecomingOldest {
      return ClusterSingletonManagerOutcome::with_phase(self.phase);
    }
    let Some(next_retry_at) = self.next_retry_at else {
      return ClusterSingletonManagerOutcome::with_phase(self.phase);
    };
    if now < next_retry_at {
      return ClusterSingletonManagerOutcome::with_phase(self.phase);
    }

    self.hand_over_retry_count = self.hand_over_retry_count.saturating_add(1);
    let max_retries = self.config.max_hand_over_retries();
    if self.hand_over_retry_count > max_retries {
      return ClusterSingletonManagerOutcome::with_effect(
        self.phase,
        ClusterSingletonManagerEffect::PublishHandOverStuck { phase: SingletonStuckPhase::BecomingOldest },
      );
    }

    self.next_retry_at = Some(schedule_after(now, self.config.hand_over_retry_interval()));
    let target_authority =
      self.previous_oldest.clone().or_else(|| self.oldest_members.first().cloned()).unwrap_or_default();
    ClusterSingletonManagerOutcome::with_effects(self.phase, vec![
      ClusterSingletonManagerEffect::SendHandOverToMe { target_authority },
      ClusterSingletonManagerEffect::ScheduleHandOverRetry,
    ])
  }

  /// Records the previous oldest authority before a topology transition.
  pub fn track_previous_oldest(&mut self, members: &[NodeRecord]) {
    let eligible = eligible_members(members, self.config.role());
    self.previous_oldest = oldest_member(&eligible).map(|record| record.authority.clone());
  }
}

fn eligible_members(members: &[NodeRecord], role: Option<&str>) -> Vec<NodeRecord> {
  members
    .iter()
    .filter(|record| record.status.is_active())
    .filter(|record| role.is_none_or(|role| record.roles.iter().any(|candidate| candidate == role)))
    .cloned()
    .collect()
}

fn still_present(members: &[NodeRecord], authority: &str) -> bool {
  members.iter().any(|record| record.authority == authority)
}

fn schedule_after(now: TimerInstant, delay: Duration) -> TimerInstant {
  if delay.is_zero() {
    return now;
  }
  let resolution_ns = now.resolution().as_nanos().max(1);
  let duration_ns = delay.as_nanos();
  let mut ticks = duration_ns / resolution_ns;
  if ticks == 0 {
    ticks = 1;
  }
  let ticks = u64::try_from(ticks).unwrap_or(u64::MAX);
  now.saturating_add_ticks(ticks)
}

/// Returns true when `left` is older than `right` using membership age order.
#[must_use]
pub fn is_older_member(left: &NodeRecord, right: &NodeRecord) -> bool {
  member_age_order(left, right) == core::cmp::Ordering::Less
}
