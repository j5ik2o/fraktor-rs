use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError};
use crate::core::FailureAction;

pub(in crate::core::stage::flow) struct RecoverLogic<Out, F> {
  pub(in crate::core::stage::flow) recover: F,
  pub(in crate::core::stage::flow) pending: Option<Out>,
}

impl<Out, F> FlowLogic for RecoverLogic<Out, F>
where
  Out: Send + Sync + 'static,
  F: FnMut(StreamError) -> Option<Out> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn handles_failures(&self) -> bool {
    true
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    let Some(value) = (self.recover)(error.clone()) else {
      return Ok(FailureAction::Propagate(error));
    };
    self.pending = Some(value);
    Ok(FailureAction::Complete)
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(value) = self.pending.take() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    self.pending.is_some()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending = None;
    Ok(())
  }
}
