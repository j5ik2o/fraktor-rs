use alloc::{vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError};
use crate::core::FailureAction;

pub(in crate::core::stage::flow) struct OnErrorContinueLogic<P, C> {
  pub(in crate::core::stage::flow) predicate:      P,
  pub(in crate::core::stage::flow) error_consumer: C,
}

impl<P, C> FlowLogic for OnErrorContinueLogic<P, C>
where
  P: FnMut(&StreamError) -> bool + Send + Sync + 'static,
  C: FnMut(&StreamError) + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn handles_failures(&self) -> bool {
    true
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    if (self.predicate)(&error) {
      (self.error_consumer)(&error);
      Ok(FailureAction::Resume)
    } else {
      Ok(FailureAction::Propagate(error))
    }
  }
}
