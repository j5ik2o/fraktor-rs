use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct CompletionTimeoutLogic<In> {
  pub(crate) duration_ticks: u64,
  pub(crate) tick_count:     u64,
  pub(crate) _pd:            PhantomData<fn(In)>,
}

impl<In> FlowLogic for CompletionTimeoutLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.tick_count > self.duration_ticks {
      return Err(StreamError::Timeout { kind: "completion", ticks: self.duration_ticks });
    }
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    Ok(())
  }
}
