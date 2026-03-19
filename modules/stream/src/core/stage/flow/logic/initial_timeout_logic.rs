use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct InitialTimeoutLogic<In> {
  pub(in crate::core::stage::flow) duration_ticks:         u64,
  pub(in crate::core::stage::flow) tick_count:             u64,
  pub(in crate::core::stage::flow) first_element_received: bool,
  pub(in crate::core::stage::flow) _pd:                    PhantomData<fn(In)>,
}

impl<In> FlowLogic for InitialTimeoutLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.first_element_received = true;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if !self.first_element_received && self.tick_count > self.duration_ticks {
      return Err(StreamError::Timeout { kind: "initial", ticks: self.duration_ticks });
    }
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.first_element_received = false;
    Ok(())
  }
}
