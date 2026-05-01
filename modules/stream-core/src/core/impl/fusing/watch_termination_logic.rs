use alloc::{vec, vec::Vec};
use core::marker::PhantomData;

use crate::core::{DynValue, FlowLogic, StreamError, materialization::StreamFuture};

pub(in crate::core) struct WatchTerminationLogic<In> {
  pub(in crate::core) completion: StreamFuture<()>,
  pub(in crate::core) _pd:        PhantomData<fn(In)>,
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
