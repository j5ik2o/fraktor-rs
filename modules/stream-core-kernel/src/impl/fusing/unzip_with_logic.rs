use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct UnzipWithLogic<In, Out, F> {
  pub(crate) func:         F,
  pub(crate) output_slots: VecDeque<usize>,
  pub(crate) _pd:          PhantomData<fn(In) -> Out>,
}

impl<In, Out, F> FlowLogic for UnzipWithLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> (Out, Out) + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let (left, right) = (self.func)(value);
    self.output_slots.push_back(0);
    self.output_slots.push_back(1);
    Ok(vec![Box::new(left) as DynValue, Box::new(right) as DynValue])
  }

  fn take_next_output_edge_slot(&mut self) -> Option<usize> {
    self.output_slots.pop_front()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.output_slots.clear();
    Ok(())
  }
}
