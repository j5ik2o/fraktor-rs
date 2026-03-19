use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct SampleLogic<In> {
  pub(in crate::core::stage::flow) interval_ticks: u64,
  pub(in crate::core::stage::flow) held:           Option<In>,
  pub(in crate::core::stage::flow) last_emit_tick: u64,
  pub(in crate::core::stage::flow) tick_count:     u64,
}

impl<In> FlowLogic for SampleLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.held = Some(value);
    Ok(Vec::new())
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.held.is_some()
      && (self.tick_count.saturating_sub(self.last_emit_tick) >= self.interval_ticks)
      && let Some(value) = self.held.take()
    {
      self.last_emit_tick = self.tick_count;
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    self.held.is_some()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.held = None;
    self.last_emit_tick = 0;
    self.tick_count = 0;
    Ok(())
  }
}
