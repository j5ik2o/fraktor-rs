use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct ScanLogic<In, Acc, F> {
  pub(in crate::core::stage::flow) initial:         Acc,
  pub(in crate::core::stage::flow) current:         Acc,
  pub(in crate::core::stage::flow) func:            F,
  pub(in crate::core::stage::flow) initial_emitted: bool,
  pub(in crate::core::stage::flow) source_done:     bool,
  pub(in crate::core::stage::flow) _pd:             PhantomData<fn(In)>,
}

impl<In, Acc, F> FlowLogic for ScanLogic<In, Acc, F>
where
  In: Send + Sync + 'static,
  Acc: Clone + Send + Sync + 'static,
  F: FnMut(Acc, In) -> Acc + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let mut outputs = Vec::new();
    if !self.initial_emitted {
      outputs.push(Box::new(self.current.clone()) as DynValue);
      self.initial_emitted = true;
    }
    let next = (self.func)(self.current.clone(), value);
    self.current = next.clone();
    outputs.push(Box::new(next) as DynValue);
    Ok(outputs)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done || self.initial_emitted {
      return Ok(Vec::new());
    }
    self.initial_emitted = true;
    Ok(vec![Box::new(self.current.clone()) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.current = self.initial.clone();
    self.initial_emitted = false;
    self.source_done = false;
    Ok(())
  }
}
