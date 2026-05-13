use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct BackpressureTimeoutLogic<In> {
  pub(crate) duration_ticks:       u64,
  pub(crate) tick_count:           u64,
  pub(crate) last_apply_tick:      u64,
  pub(crate) has_received_element: bool,
  pub(crate) _pd:                  PhantomData<fn(In)>,
}

impl<In> FlowLogic for BackpressureTimeoutLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.has_received_element = true;
    self.last_apply_tick = self.tick_count;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.has_received_element && self.tick_count.saturating_sub(self.last_apply_tick) > self.duration_ticks {
      return Err(StreamError::Timeout { kind: "backpressure", ticks: self.duration_ticks });
    }
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.last_apply_tick = 0;
    self.has_received_element = false;
    Ok(())
  }
}
