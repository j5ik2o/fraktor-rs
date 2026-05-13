use alloc::{format, vec::Vec};

use super::buffered_edge::BufferedEdge;
use crate::{
  StageDefinition,
  shape::PortId,
  snapshot::{ConnectionSnapshot, LogicSnapshot, RunningInterpreter},
};

/// Builds interpreter snapshots from stage and edge state.
pub(crate) struct InterpreterSnapshotBuilder<'a> {
  stages: &'a [StageDefinition],
}

impl<'a> InterpreterSnapshotBuilder<'a> {
  /// Creates a snapshot builder for the provided stage definitions.
  #[must_use]
  pub(crate) const fn new(stages: &'a [StageDefinition]) -> Self {
    Self { stages }
  }

  /// Builds a running interpreter snapshot from the current edges.
  #[must_use]
  pub(crate) fn build(&self, edges: &[BufferedEdge]) -> RunningInterpreter {
    let logics: Vec<LogicSnapshot> = self
      .stages
      .iter()
      .enumerate()
      .map(|(index, stage)| LogicSnapshot::new(index as u32, format!("{:?}", stage.kind()), stage.attributes().clone()))
      .collect();

    let connections: Vec<ConnectionSnapshot> = edges
      .iter()
      .enumerate()
      .filter_map(|(edge_index, edge)| {
        let in_index = self.stage_index_for_outlet(edge.from())?;
        let out_index = self.stage_index_for_inlet(edge.to())?;
        let in_logic = logics.get(in_index).cloned()?;
        let out_logic = logics.get(out_index).cloned()?;
        Some(ConnectionSnapshot::new(edge_index as u32, in_logic, out_logic, edge.connection_state()))
      })
      .collect();

    let running_logics_count = self.stages.len() as u32;
    let stopped_logics: Vec<LogicSnapshot> = Vec::new();

    RunningInterpreter::new(logics, connections, running_logics_count, stopped_logics)
  }

  fn stage_index_for_outlet(&self, outlet: PortId) -> Option<usize> {
    self
      .stages
      .iter()
      .enumerate()
      .find_map(|(index, stage)| stage.outlet().filter(|stage_outlet| *stage_outlet == outlet).map(|_| index))
  }

  fn stage_index_for_inlet(&self, inlet: PortId) -> Option<usize> {
    self
      .stages
      .iter()
      .enumerate()
      .find_map(|(index, stage)| stage.inlet().filter(|stage_inlet| *stage_inlet == inlet).map(|_| index))
  }
}
