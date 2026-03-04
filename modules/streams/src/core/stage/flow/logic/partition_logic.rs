use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct PartitionLogic<In, F> {
  pub(in crate::core::stage::flow) predicate:    F,
  pub(in crate::core::stage::flow) output_slots: VecDeque<usize>,
  pub(in crate::core::stage::flow) _pd:          PhantomData<fn(In)>,
}

impl<In, F> FlowLogic for PartitionLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let slot = if (self.predicate)(&value) { 0 } else { 1 };
    self.output_slots.push_back(slot);
    Ok(vec![Box::new(value) as DynValue])
  }

  fn take_next_output_edge_slot(&mut self) -> Option<usize> {
    self.output_slots.pop_front()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.output_slots.clear();
    Ok(())
  }
}
