use super::{super::failure_disposition::FailureDisposition, GraphInterpreter};
use crate::core::{FailureAction, StageDefinition, StageKind, StreamError, SupervisionStrategy};

impl GraphInterpreter {
  pub(super) fn tick_restart_windows(&mut self) -> Result<(), StreamError> {
    for (stage_index, stage) in self.stages.iter_mut().enumerate() {
      match stage {
        | StageDefinition::Source(source) => {
          if let Some(restart) = &mut source.restart
            && restart.tick(self.tick_count)
          {
            source.logic.on_restart()?;
          }
        },
        | StageDefinition::Flow(flow) => {
          if let Some(restart) = &mut flow.restart
            && restart.tick(self.tick_count)
          {
            flow.logic.on_restart()?;
          }
        },
        | StageDefinition::Sink(sink) => {
          if let Some(restart) = &mut sink.restart
            && restart.tick(self.tick_count)
          {
            sink.logic.on_restart()?;
            sink.logic.on_start(&mut self.demand)?;
            if let Some(pos) = self.sink_indices.iter().position(|&idx| idx == stage_index) {
              self.sink_upstream_notified[pos] = false;
            }
          }
        },
      }
    }
    Ok(())
  }

  pub(super) fn source_restart_waiting(&self) -> bool {
    for source_index in &self.source_indices {
      let StageDefinition::Source(source) = &self.stages[*source_index] else {
        return false;
      };
      if source.restart.as_ref().map(|restart| restart.is_waiting()).unwrap_or(false) {
        return true;
      }
    }
    false
  }

  pub(super) fn source_restart_waiting_at(&self, source_position: usize) -> bool {
    let source_index = self.source_indices[source_position];
    let StageDefinition::Source(source) = &self.stages[source_index] else {
      return false;
    };
    source.restart.as_ref().map(|restart| restart.is_waiting()).unwrap_or(false)
  }

  pub(super) fn flow_restart_waiting(&self, stage_index: usize) -> bool {
    let StageDefinition::Flow(flow) = &self.stages[stage_index] else {
      return false;
    };
    flow.restart.as_ref().map(|restart| restart.is_waiting()).unwrap_or(false)
  }

  pub(super) fn sink_restart_waiting(&self) -> bool {
    for sink_index in &self.sink_indices {
      if self.sink_restart_waiting_at(*sink_index) {
        return true;
      }
    }
    false
  }

  pub(super) fn sink_restart_waiting_at(&self, sink_index: usize) -> bool {
    let StageDefinition::Sink(sink) = &self.stages[sink_index] else {
      return false;
    };
    sink.restart.as_ref().map(|restart| restart.is_waiting()).unwrap_or(false)
  }

  pub(super) fn apply_failure_action(
    &mut self,
    handler_stage_index: usize,
    action: FailureAction,
  ) -> Result<FailureDisposition, StreamError> {
    match action {
      | FailureAction::Propagate(error) => Ok(FailureDisposition::Fail(error)),
      | FailureAction::Resume => Ok(FailureDisposition::Continue),
      | FailureAction::Complete => {
        self.shutdown_flow_stage(handler_stage_index)?;
        Ok(FailureDisposition::Continue)
      },
    }
  }

  pub(super) fn propagate_failure_to_downstream(
    &mut self,
    stage_index: usize,
    error: StreamError,
  ) -> Result<(bool, FailureDisposition), StreamError> {
    let Some(outlet) = self.stages[stage_index].outlet() else {
      return Ok((false, FailureDisposition::Fail(error)));
    };
    let Ok(outgoing_edges) = self.outgoing_edge_indices(outlet) else {
      return Ok((false, FailureDisposition::Fail(error)));
    };
    if outgoing_edges.len() != 1 {
      return Ok((false, FailureDisposition::Fail(error)));
    }
    let edge_index = outgoing_edges[0];
    if self.connections.edge_closed(edge_index) {
      return Ok((false, FailureDisposition::Fail(error)));
    }
    let Some(next_stage_index) = self.stage_index_for_inlet(self.connections.edge_to(edge_index)) else {
      return Ok((false, FailureDisposition::Fail(error)));
    };
    let action = {
      let StageDefinition::Flow(flow) = &mut self.stages[next_stage_index] else {
        return Ok((false, FailureDisposition::Fail(error)));
      };
      let reports_failure_handling = flow.logic.handles_failures();
      let action = flow.logic.on_failure(error)?;
      debug_assert!(
        reports_failure_handling || matches!(action, FailureAction::Propagate(_)),
        "FlowLogic returning Resume/Complete should report handles_failures() = true"
      );
      action
    };
    match action {
      | FailureAction::Resume | FailureAction::Complete => {
        let disposition = self.apply_failure_action(next_stage_index, action)?;
        Ok((true, disposition))
      },
      | FailureAction::Propagate(next_error) => {
        let (touched_downstream, disposition) = self.propagate_failure_to_downstream(next_stage_index, next_error)?;
        Ok((touched_downstream, disposition))
      },
    }
  }

  pub(super) fn handle_source_failure(
    &mut self,
    source_position: usize,
    error: StreamError,
  ) -> Result<FailureDisposition, StreamError> {
    let source_index = self.source_indices[source_position];
    let (handled, disposition) = self.propagate_failure_to_downstream(source_index, error.clone())?;
    if handled {
      return Ok(disposition);
    }
    let fallback_error = match disposition {
      | FailureDisposition::Fail(next_error) => next_error,
      | FailureDisposition::Continue | FailureDisposition::Complete => error,
    };
    let StageDefinition::Source(source) = &mut self.stages[source_index] else {
      return Ok(FailureDisposition::Fail(StreamError::InvalidConnection));
    };
    if let Some(restart) = &mut source.restart {
      if restart.schedule(self.tick_count) {
        return Ok(FailureDisposition::Continue);
      }
      return if restart.complete_on_max_restarts() {
        Ok(FailureDisposition::Complete)
      } else {
        Ok(FailureDisposition::Fail(fallback_error))
      };
    }
    match source.supervision {
      | SupervisionStrategy::Stop => Ok(FailureDisposition::Fail(fallback_error)),
      | SupervisionStrategy::Resume => Ok(FailureDisposition::Continue),
      | SupervisionStrategy::Restart => {
        source.logic.on_restart()?;
        Ok(FailureDisposition::Continue)
      },
    }
  }

  pub(super) fn handle_flow_failure(
    &mut self,
    stage_index: usize,
    error: &StreamError,
  ) -> Result<FailureDisposition, StreamError> {
    let self_action = {
      let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
        return Ok(FailureDisposition::Fail(StreamError::InvalidConnection));
      };
      let reports_failure_handling = flow.logic.handles_failures();
      let action = flow.logic.on_failure(error.clone())?;
      debug_assert!(
        reports_failure_handling || matches!(action, FailureAction::Propagate(_)),
        "FlowLogic returning Resume/Complete should report handles_failures() = true"
      );
      action
    };
    match self_action {
      | FailureAction::Resume | FailureAction::Complete => {
        let disposition = self.apply_failure_action(stage_index, self_action)?;
        Ok(disposition)
      },
      | FailureAction::Propagate(error) => {
        let (handled_downstream, disposition) = self.propagate_failure_to_downstream(stage_index, error.clone())?;
        if handled_downstream {
          return Ok(disposition);
        }
        let fallback_error = match disposition {
          | FailureDisposition::Fail(next_error) => next_error,
          | FailureDisposition::Continue | FailureDisposition::Complete => error,
        };
        let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
          return Ok(FailureDisposition::Fail(StreamError::InvalidConnection));
        };
        if let Some(restart) = &mut flow.restart {
          if restart.schedule(self.tick_count) {
            return Ok(FailureDisposition::Continue);
          }
          return if restart.complete_on_max_restarts() {
            Ok(FailureDisposition::Complete)
          } else {
            Ok(FailureDisposition::Fail(fallback_error))
          };
        }
        match flow.supervision {
          | SupervisionStrategy::Stop => Ok(FailureDisposition::Fail(fallback_error)),
          | SupervisionStrategy::Resume => Ok(FailureDisposition::Continue),
          | SupervisionStrategy::Restart => {
            if matches!(flow.kind, StageKind::FlowSplitWhen | StageKind::FlowSplitAfter) {
              return Ok(FailureDisposition::Continue);
            }
            flow.logic.on_restart()?;
            Ok(FailureDisposition::Continue)
          },
        }
      },
    }
  }

  pub(super) fn handle_sink_failure(
    &mut self,
    sink_index: usize,
    error: StreamError,
  ) -> Result<FailureDisposition, StreamError> {
    let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
      return Ok(FailureDisposition::Fail(StreamError::InvalidConnection));
    };
    if let Some(restart) = &mut sink.restart {
      if restart.schedule(self.tick_count) {
        self.demand.request(1)?;
        return Ok(FailureDisposition::Continue);
      }
      return if restart.complete_on_max_restarts() {
        Ok(FailureDisposition::Complete)
      } else {
        Ok(FailureDisposition::Fail(error))
      };
    }
    match sink.supervision {
      | SupervisionStrategy::Stop => Ok(FailureDisposition::Fail(error)),
      | SupervisionStrategy::Resume => {
        self.demand.request(1)?;
        Ok(FailureDisposition::Continue)
      },
      | SupervisionStrategy::Restart => {
        sink.logic.on_restart()?;
        sink.logic.on_start(&mut self.demand)?;
        if let Some(pos) = self.sink_indices.iter().position(|&idx| idx == sink_index) {
          self.sink_upstream_notified[pos] = false;
        }
        Ok(FailureDisposition::Continue)
      },
    }
  }
}
