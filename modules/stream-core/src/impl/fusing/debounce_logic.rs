use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct DebounceLogic<In> {
  pub(crate) silence_ticks:     u64,
  pub(crate) held:              Option<In>,
  pub(crate) last_receive_tick: u64,
  pub(crate) tick_count:        u64,
}

impl<In> FlowLogic for DebounceLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.held = Some(value);
    self.last_receive_tick = self.tick_count;
    Ok(Vec::new())
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.held.is_some()
      && (self.tick_count.saturating_sub(self.last_receive_tick) >= self.silence_ticks)
      && let Some(value) = self.held.take()
    {
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
    self.last_receive_tick = 0;
    self.tick_count = 0;
    Ok(())
  }
}
