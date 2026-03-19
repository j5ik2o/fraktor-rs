use alloc::{vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError};
use crate::core::FailureAction;

pub(in crate::core::stage::flow) struct OnErrorCompleteLogic<F> {
  pub(in crate::core::stage::flow) predicate: F,
}

impl<F> FlowLogic for OnErrorCompleteLogic<F>
where
  F: FnMut(&StreamError) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn handles_failures(&self) -> bool {
    true
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    if (self.predicate)(&error) { Ok(FailureAction::Complete) } else { Ok(FailureAction::Propagate(error)) }
  }
}
