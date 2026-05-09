use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct UnzipLogic<In> {
  pub(crate) output_slots: VecDeque<usize>,
  pub(crate) _pd:          PhantomData<fn(In)>,
}

impl<In> FlowLogic for UnzipLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let (left, right) = downcast_value::<(In, In)>(input)?;
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
