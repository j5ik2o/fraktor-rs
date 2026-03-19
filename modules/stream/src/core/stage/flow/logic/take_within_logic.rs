use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct TakeWithinLogic<In> {
  pub(in crate::core::stage::flow) duration_ticks:     u64,
  pub(in crate::core::stage::flow) tick_count:         u64,
  pub(in crate::core::stage::flow) expired:            bool,
  pub(in crate::core::stage::flow) shutdown_requested: bool,
  pub(in crate::core::stage::flow) _pd:                PhantomData<fn(In)>,
}

impl<In> FlowLogic for TakeWithinLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.expired {
      return Ok(Vec::new());
    }
    if self.tick_count > self.duration_ticks {
      self.expired = true;
      self.shutdown_requested = true;
      return Ok(Vec::new());
    }
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.tick_count > self.duration_ticks {
      self.expired = true;
      self.shutdown_requested = true;
    }
    Ok(())
  }

  fn take_shutdown_request(&mut self) -> bool {
    let requested = self.shutdown_requested;
    self.shutdown_requested = false;
    requested
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.expired = false;
    self.shutdown_requested = false;
    Ok(())
  }
}
