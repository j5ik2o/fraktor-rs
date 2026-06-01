//! Evaluation context for downing decisions.

#[cfg(test)]
#[path = "downing_decision_context_test.rs"]
mod tests;

use alloc::string::String;

use fraktor_remote_core_rs::address::UniqueAddress;
use fraktor_utils_core_rs::time::TimerInstant;

use super::{DowningInput, FailureObservation};
use crate::membership::{IndirectConnectionEvidence, MembershipSnapshot, NodeRecord, ReachabilityStatus};

const MISSING_REACHABILITY_EVIDENCE: &str = "reachability evidence is required for membership evaluation";

/// Immutable input snapshot consumed by downing and split-brain evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DowningDecisionContext {
  input:           DowningDecisionContextInput,
  evaluation_time: TimerInstant,
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
      },
      evaluation_time,
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
      },
      evaluation_time,
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
      },
      | DowningInput::IndirectConnectionEvidence(indirect_connection_evidence) => Self {
        input: DowningDecisionContextInput::IndirectConnectionEvidence {
          indirect_connection_evidence: indirect_connection_evidence.clone(),
        },
        evaluation_time,
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
        },
        evaluation_time,
      },
      | DowningInput::IndirectConnectionEvidence(indirect_connection_evidence) => Self {
        input: DowningDecisionContextInput::Membership {
          snapshot,
          indirect_connection_evidence: Some(indirect_connection_evidence.clone()),
          failure_observation: None,
        },
        evaluation_time,
      },
    }
  }

  /// Creates an evaluation context for an explicit down command.
  #[must_use]
  pub fn from_explicit_down(authority: &str, evaluation_time: TimerInstant) -> Self {
    Self { input: DowningDecisionContextInput::ExplicitDown { authority: String::from(authority) }, evaluation_time }
  }

  /// Returns the time attached to this evaluation input.
  #[must_use]
  pub const fn evaluation_time(&self) -> TimerInstant {
    self.evaluation_time
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
    Some(snapshot.reachability.aggregate_status(unique_address))
  }

  /// Returns true when membership evaluation cannot proceed without reachability evidence.
  #[must_use]
  pub fn requires_reachability_evidence(&self) -> bool {
    match &self.input {
      | DowningDecisionContextInput::Membership { snapshot, indirect_connection_evidence, .. } => {
        snapshot.reachability.records.is_empty()
          && snapshot.reachability.observer_versions.is_empty()
          && indirect_connection_evidence.is_none()
      },
      | DowningDecisionContextInput::FailureObservation { .. } => true,
      | DowningDecisionContextInput::ExplicitDown { .. }
      | DowningDecisionContextInput::IndirectConnectionEvidence { .. } => false,
    }
  }

  /// Returns a defer reason when the context has insufficient evidence for membership evaluation.
  #[must_use]
  pub fn defer_reason(&self) -> Option<&'static str> {
    if self.requires_reachability_evidence() { Some(MISSING_REACHABILITY_EVIDENCE) } else { None }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DowningDecisionContextInput {
  Membership {
    snapshot:                     MembershipSnapshot,
    indirect_connection_evidence: Option<IndirectConnectionEvidence>,
    failure_observation:          Option<FailureObservation>,
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
