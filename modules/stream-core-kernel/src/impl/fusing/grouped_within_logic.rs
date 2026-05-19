use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct GroupedWithinLogic<In> {
  pub(crate) size:              usize,
  pub(crate) duration_ticks:    u64,
  pub(crate) tick_count:        u64,
  pub(crate) window_start_tick: Option<u64>,
  pub(crate) current:           Vec<In>,
  pub(crate) pending:           VecDeque<Vec<In>>,
}

impl<In> GroupedWithinLogic<In>
where
  In: Send + Sync + 'static,
{
  fn tick_window_expired(&self) -> bool {
    self
      .window_start_tick
      .is_some_and(|window_start_tick| self.tick_count >= window_start_tick.saturating_add(self.duration_ticks))
  }

  fn flush_current(&mut self) {
    if self.current.is_empty() {
      return;
    }
    self.pending.push_back(core::mem::take(&mut self.current));
    self.window_start_tick = None;
  }
}

impl<In> FlowLogic for GroupedWithinLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.size == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    if self.current.is_empty() {
      self.window_start_tick = Some(self.tick_count);
    }
    self.current.push(value);
    if self.current.len() >= self.size {
      self.flush_current();
    }
    self.drain_pending()
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.tick_window_expired() {
      self.flush_current();
    }
    Ok(())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.flush_current();
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(values) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(values) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.window_start_tick = None;
    self.current.clear();
    self.pending.clear();
    Ok(())
  }
}
