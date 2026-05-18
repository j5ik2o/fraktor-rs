use alloc::{vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError};
use crate::FailureAction;

pub(crate) struct MapErrorLogic<F> {
  pub(crate) mapper: F,
}

impl<F> FlowLogic for MapErrorLogic<F>
where
  F: FnMut(StreamError) -> StreamError + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn handles_failures(&self) -> bool {
    true
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    Ok(FailureAction::Propagate((self.mapper)(error)))
  }
}
