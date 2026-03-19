use alloc::{vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamCompletion, StreamError};

pub(in crate::core::stage::flow) struct WatchTerminationLogic<In> {
  pub(in crate::core::stage::flow) completion: StreamCompletion<()>,
  pub(in crate::core::stage::flow) _pd:        PhantomData<fn(In)>,
}

impl<In> FlowLogic for WatchTerminationLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(()));
    Ok(())
  }
}
