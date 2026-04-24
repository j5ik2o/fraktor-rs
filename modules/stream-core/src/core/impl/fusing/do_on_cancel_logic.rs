use alloc::{vec, vec::Vec};
use core::marker::PhantomData;

use crate::core::{DownstreamCancelAction, DynValue, FlowLogic, StreamError};

/// Flow logic that invokes a callback once when downstream cancels.
pub(in crate::core) struct DoOnCancelLogic<In, F> {
  pub(in crate::core) callback: F,
  pub(in crate::core) fired:    bool,
  pub(in crate::core) _pd:      PhantomData<fn(In)>,
}

impl<In, F> FlowLogic for DoOnCancelLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut() + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    if !self.fired {
      (self.callback)();
      self.fired = true;
    }
    Ok(DownstreamCancelAction::Propagate)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.fired = false;
    Ok(())
  }
}
