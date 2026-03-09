use alloc::{boxed::Box, vec, vec::Vec};

use super::{
  super::super::{DynValue, FlowLogic, Source, StreamError, StreamNotUsed},
  SecondarySourceBridge,
};
use crate::core::FailureAction;

pub(in crate::core::stage::flow) struct RecoverWithRetriesLogic<Out, F> {
  pub(in crate::core::stage::flow) max_retries:     Option<usize>,
  pub(in crate::core::stage::flow) retries_left:    Option<usize>,
  pub(in crate::core::stage::flow) recover:         F,
  pub(in crate::core::stage::flow) recovery_source: Option<SecondarySourceBridge<Out>>,
}

impl<Out, F> RecoverWithRetriesLogic<Out, F>
where
  Out: Send + Sync + 'static,
  F: FnMut(StreamError) -> Option<Source<Out, StreamNotUsed>> + Send + Sync + 'static,
{
  fn begin_recovery(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    if matches!(self.retries_left, Some(0)) {
      return Ok(FailureAction::Propagate(error));
    }
    if let Some(retries_left) = self.retries_left.as_mut() {
      *retries_left = retries_left.saturating_sub(1);
    }
    let Some(source) = (self.recover)(error.clone()) else {
      return Ok(FailureAction::Propagate(error));
    };
    self.recovery_source = Some(SecondarySourceBridge::new(source)?);
    Ok(FailureAction::Complete)
  }
}

impl<Out, F> FlowLogic for RecoverWithRetriesLogic<Out, F>
where
  Out: Send + Sync + 'static,
  F: FnMut(StreamError) -> Option<Source<Out, StreamNotUsed>> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn handles_failures(&self) -> bool {
    true
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    self.begin_recovery(error)
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(runtime) = self.recovery_source.as_mut() else {
      return Ok(Vec::new());
    };
    let Some(value) = runtime.poll_next()? else {
      if !runtime.has_pending_output() {
        self.recovery_source = None;
      }
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    self.recovery_source.as_ref().is_some_and(SecondarySourceBridge::has_pending_output)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.retries_left = self.max_retries;
    self.recovery_source = None;
    Ok(())
  }
}
