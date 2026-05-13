use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::dsl::StatefulMapConcatAccumulator;

pub(crate) struct StatefulMapConcatAccumulatorLogic<In, Out, Factory, Acc> {
  pub(crate) factory:     Factory,
  pub(crate) accumulator: Acc,
  pub(crate) source_done: bool,
  pub(crate) pending:     Vec<DynValue>,
  pub(crate) _pd:         PhantomData<fn(In) -> Out>,
}

impl<In, Out, Factory, Acc> FlowLogic for StatefulMapConcatAccumulatorLogic<In, Out, Factory, Acc>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Factory: FnMut() -> Acc + Send + Sync + 'static,
  Acc: StatefulMapConcatAccumulator<In, Out> + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let outputs = self.accumulator.apply(value);
    Ok(outputs.into_iter().map(|v| Box::new(v) as DynValue).collect())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    let trailing = self.accumulator.on_complete();
    for v in trailing {
      self.pending.push(Box::new(v) as DynValue);
    }
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(core::mem::take(&mut self.pending))
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.accumulator = (self.factory)();
    self.source_done = false;
    self.pending.clear();
    Ok(())
  }
}
