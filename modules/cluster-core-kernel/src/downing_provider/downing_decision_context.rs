//! Evaluation context for downing decisions.

#[cfg(test)]
#[path = "downing_decision_context_test.rs"]
mod tests;

use alloc::string::String;
use core::time::Duration;

use fraktor_remote_core_rs::address::UniqueAddress;
use fraktor_utils_core_rs::time::TimerInstant;

use super::{DowningInput, FailureObservation};
use crate::membership::{IndirectConnectionEvidence, MembershipSnapshot, NodeRecord, ReachabilityStatus};

const MISSING_REACHABILITY_EVIDENCE: &str = "reachability evidence is required for membership evaluation";
const LOCAL_REACHABILITY_OBSERVER_REQUIRED: &str =
  "local reachability observer is required for observed membership evaluation";

/// Immutable input snapshot consumed by downing and split-brain evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DowningDecisionContext {
  input:           DowningDecisionContextInput,
  evaluation_time: TimerInstant,
  unstable_since:  TimerInstant,
}

impl DowningDecisionContext {
  /// Creates an evaluation context from a membership snapshot.
  #[must_use]
  pub const fn from_membership_snapshot(snapshot: MembershipSnapshot, evaluation_time: TimerInstant) -> Self {
    Self {
      input: DowningDecisionContextInput::Membership {
        snapshot,
        indirect_connection_evidence: None,
        failure_observation: None,
        reachability_observer: None,
      },
      evaluation_time,
      unstable_since: evaluation_time,
    }
  }

  /// Creates an evaluation context from a membership snapshot and indirect reachability evidence.
  #[must_use]
  pub const fn from_membership_snapshot_with_indirect_evidence(
    snapshot: MembershipSnapshot,
    indirect_connection_evidence: IndirectConnectionEvidence,
    evaluation_time: TimerInstant,
  ) -> Self {
    Self {
      input: DowningDecisionContextInput::Membership {
        snapshot,
        indirect_connection_evidence: Some(indirect_connection_evidence),
        failure_observation: None,
        reachability_observer: None,
      },
      evaluation_time,
      unstable_since: evaluation_time,
    }
  }

  /// Creates an evaluation context from an existing downing input.
  #[must_use]
  pub fn from_downing_input(input: &DowningInput, evaluation_time: TimerInstant) -> Self {
    match input {
      | DowningInput::ExplicitDown { authority } => Self::from_explicit_down(authority, evaluation_time),
      | DowningInput::FailureObservation(observation) => Self {
        input: DowningDecisionContextInput::FailureObservation { observation: observation.clone() },
        evaluation_time,
        unstable_since: evaluation_time,
      },
      | DowningInput::IndirectConnectionEvidence(indirect_connection_evidence) => Self {
        input: DowningDecisionContextInput::IndirectConnectionEvidence {
          indirect_connection_evidence: indirect_connection_evidence.clone(),
        },
        evaluation_time,
        unstable_since: evaluation_time,
      },
    }
  }

  /// Creates an evaluation context from an existing downing input and membership snapshot.
  #[must_use]
  pub fn from_downing_input_with_membership_snapshot(
    input: &DowningInput,
    snapshot: MembershipSnapshot,
    evaluation_time: TimerInstant,
  ) -> Self {
    match input {
      | DowningInput::ExplicitDown { authority } => Self::from_explicit_down(authority, evaluation_time),
      | DowningInput::FailureObservation(observation) => Self {
        input: DowningDecisionContextInput::Membership {
          snapshot,
          indirect_connection_evidence: None,
          failure_observation: Some(observation.clone()),
          reachability_observer: None,
        },
        evaluation_time,
        unstable_since: evaluation_time,
      },
      | DowningInput::IndirectConnectionEvidence(indirect_connection_evidence) => Self {
        input: DowningDecisionContextInput::Membership {
          snapshot,
          indirect_connection_evidence: Some(indirect_connection_evidence.clone()),
          failure_observation: None,
          reachability_observer: None,
        },
        evaluation_time,
        unstable_since: evaluation_time,
      },
    }
  }

  /// Creates an evaluation context for an explicit down command.
  #[must_use]
  pub fn from_explicit_down(authority: &str, evaluation_time: TimerInstant) -> Self {
    Self {
      input: DowningDecisionContextInput::ExplicitDown { authority: String::from(authority) },
      evaluation_time,
      unstable_since: evaluation_time,
    }
  }

  /// Returns this context with a local reachability observer for partition evaluation.
  #[must_use]
  pub fn with_reachability_observer(mut self, observer: UniqueAddress) -> Self {
    if let DowningDecisionContextInput::Membership { reachability_observer, .. } = &mut self.input {
      *reachability_observer = Some(observer);
    }
    self
  }

  /// Returns this context with the time at which the membership became unstable.
  #[must_use]
  pub const fn with_unstable_since(mut self, unstable_since: TimerInstant) -> Self {
    self.unstable_since = unstable_since;
    self
  }

  /// Returns the time attached to this evaluation input.
  #[must_use]
  pub const fn evaluation_time(&self) -> TimerInstant {
    self.evaluation_time
  }

  /// Returns when the evaluated membership first became unstable.
  #[must_use]
  pub const fn unstable_since(&self) -> TimerInstant {
    self.unstable_since
  }

  /// Returns elapsed unstable duration at evaluation time.
  #[must_use]
  pub fn unstable_duration(&self) -> Duration {
    let evaluation_duration = ticks_to_duration(self.evaluation_time.ticks(), self.evaluation_time.resolution());
    let unstable_since_duration = ticks_to_duration(self.unstable_since.ticks(), self.unstable_since.resolution());
    evaluation_duration.saturating_sub(unstable_since_duration)
  }

  /// Returns the membership snapshot when this context was built from membership state.
  #[must_use]
  pub const fn membership_snapshot(&self) -> Option<&MembershipSnapshot> {
    match &self.input {
      | DowningDecisionContextInput::Membership { snapshot, .. } => Some(snapshot),
      | DowningDecisionContextInput::ExplicitDown { .. }
      | DowningDecisionContextInput::FailureObservation { .. }
      | DowningDecisionContextInput::IndirectConnectionEvidence { .. } => None,
    }
  }

  /// Returns indirect connection evidence when it was provided for this evaluation.
  #[must_use]
  pub const fn indirect_connection_evidence(&self) -> Option<&IndirectConnectionEvidence> {
    match &self.input {
      | DowningDecisionContextInput::Membership { indirect_connection_evidence, .. } => {
        indirect_connection_evidence.as_ref()
      },
      | DowningDecisionContextInput::IndirectConnectionEvidence { indirect_connection_evidence } => {
        Some(indirect_connection_evidence)
      },
      | DowningDecisionContextInput::ExplicitDown { .. } | DowningDecisionContextInput::FailureObservation { .. } => {
        None
      },
    }
  }

  /// Returns the failure observation when this context came from failure evidence.
  #[must_use]
  pub const fn failure_observation(&self) -> Option<&FailureObservation> {
    match &self.input {
      | DowningDecisionContextInput::Membership { failure_observation, .. } => failure_observation.as_ref(),
      | DowningDecisionContextInput::FailureObservation { observation } => Some(observation),
      | DowningDecisionContextInput::ExplicitDown { .. }
      | DowningDecisionContextInput::IndirectConnectionEvidence { .. } => None,
    }
  }

  /// Returns the observer whose reachability row should be used for membership partitioning.
  #[must_use]
  pub const fn reachability_observer(&self) -> Option<&UniqueAddress> {
    match &self.input {
      | DowningDecisionContextInput::Membership { reachability_observer, .. } => reachability_observer.as_ref(),
      | DowningDecisionContextInput::ExplicitDown { .. }
      | DowningDecisionContextInput::FailureObservation { .. }
      | DowningDecisionContextInput::IndirectConnectionEvidence { .. } => None,
    }
  }

  /// Returns the explicit down authority when this context came from an explicit command.
  #[must_use]
  pub const fn explicit_down_authority(&self) -> Option<&str> {
    match &self.input {
      | DowningDecisionContextInput::ExplicitDown { authority } => Some(authority.as_str()),
      | DowningDecisionContextInput::Membership { .. }
      | DowningDecisionContextInput::FailureObservation { .. }
      | DowningDecisionContextInput::IndirectConnectionEvidence { .. } => None,
    }
  }

  /// Returns the member record for the given unique address.
  #[must_use]
  pub fn member_record(&self, unique_address: &UniqueAddress) -> Option<&NodeRecord> {
    let snapshot = self.membership_snapshot()?;
    snapshot.entries.iter().find(|record| &record.unique_address == unique_address)
  }

  /// Returns the aggregate reachability status for the given member.
  #[must_use]
  pub fn reachability_status(&self, unique_address: &UniqueAddress) -> Option<ReachabilityStatus> {
    let snapshot = self.membership_snapshot()?;
    if let Some(observer) = self.reachability_observer() {
      return snapshot.reachability.observed_status(observer, unique_address);
    }
    if self.indirect_connection_evidence().is_some_and(|evidence| &evidence.subject == unique_address) {
      return Some(ReachabilityStatus::Unreachable);
    }
    Some(snapshot.reachability.aggregate_status(unique_address))
  }

  /// Returns true when membership evaluation cannot proceed without reachability evidence.
  #[must_use]
  pub fn requires_reachability_evidence(&self) -> bool {
    match &self.input {
      | DowningDecisionContextInput::Membership {
        snapshot,
        indirect_connection_evidence,
        reachability_observer,
        ..
      } => {
        let lacks_reachability =
          snapshot.reachability.records.is_empty() && snapshot.reachability.observer_versions.is_empty();
        let lacks_observer_row =
          reachability_observer.as_ref().is_some_and(|observer| !snapshot.reachability.has_observer(observer));
        (lacks_reachability && indirect_connection_evidence.is_none()) || lacks_observer_row
      },
      | DowningDecisionContextInput::FailureObservation { .. } => true,
      | DowningDecisionContextInput::ExplicitDown { .. }
      | DowningDecisionContextInput::IndirectConnectionEvidence { .. } => false,
    }
  }

  /// Returns true when observed reachability exists but no local observer was selected.
  #[must_use]
  pub fn requires_reachability_observer(&self) -> bool {
    match &self.input {
      | DowningDecisionContextInput::Membership { snapshot, reachability_observer, .. } => {
        reachability_observer.is_none() && !snapshot.reachability.observer_versions.is_empty()
      },
      | DowningDecisionContextInput::ExplicitDown { .. }
      | DowningDecisionContextInput::FailureObservation { .. }
      | DowningDecisionContextInput::IndirectConnectionEvidence { .. } => false,
    }
  }

  /// Returns a defer reason when the context has insufficient evidence for membership evaluation.
  #[must_use]
  pub fn defer_reason(&self) -> Option<&'static str> {
    if self.requires_reachability_evidence() {
      Some(MISSING_REACHABILITY_EVIDENCE)
    } else if self.requires_reachability_observer() {
      Some(LOCAL_REACHABILITY_OBSERVER_REQUIRED)
    } else {
      None
    }
  }
}

fn ticks_to_duration(ticks: u64, resolution: Duration) -> Duration {
  let nanos = resolution.as_nanos().saturating_mul(u128::from(ticks));
  let clamped = nanos.min(u128::from(u64::MAX));
  Duration::from_nanos(clamped as u64)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DowningDecisionContextInput {
  Membership {
    snapshot:                     MembershipSnapshot,
    indirect_connection_evidence: Option<IndirectConnectionEvidence>,
    failure_observation:          Option<FailureObservation>,
    reachability_observer:        Option<UniqueAddress>,
  },
  ExplicitDown {
    authority: String,
  },
  FailureObservation {
    observation: FailureObservation,
  },
  IndirectConnectionEvidence {
    indirect_connection_evidence: IndirectConnectionEvidence,
  },
}
