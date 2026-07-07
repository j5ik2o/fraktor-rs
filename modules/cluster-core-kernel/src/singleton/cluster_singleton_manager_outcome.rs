//! Outcome produced by applying input to the Cluster Singleton manager state machine.

use alloc::vec::Vec;

use super::{
  cluster_singleton_manager_effect::ClusterSingletonManagerEffect,
  cluster_singleton_manager_phase::ClusterSingletonManagerPhase,
};

/// Outcome produced by applying manager input to the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSingletonManagerOutcome {
  /// New manager phase after the transition.
  pub phase:   ClusterSingletonManagerPhase,
  /// Effects for the runtime driver to execute.
  pub effects: Vec<ClusterSingletonManagerEffect>,
}

impl ClusterSingletonManagerOutcome {
  pub(crate) const fn with_phase(phase: ClusterSingletonManagerPhase) -> Self {
    Self { phase, effects: Vec::new() }
  }

  pub(crate) fn with_effect(phase: ClusterSingletonManagerPhase, effect: ClusterSingletonManagerEffect) -> Self {
    Self { phase, effects: Vec::from([effect]) }
  }

  pub(crate) const fn with_effects(
    phase: ClusterSingletonManagerPhase,
    effects: Vec<ClusterSingletonManagerEffect>,
  ) -> Self {
    Self { phase, effects }
  }
}
