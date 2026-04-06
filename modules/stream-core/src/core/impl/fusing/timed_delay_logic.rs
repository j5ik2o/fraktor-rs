use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core) enum DelayMode {
  PerElement { delay_ticks: u64 },
  Initial { initial_delay_ticks: u64 },
}

pub(in crate::core) struct TimedPendingEntry<In> {
  pub(in crate::core) ready_at: u64,
  pub(in crate::core) value:    In,
}

pub(in crate::core) struct TimedDelayLogic<In> {
  pub(in crate::core) mode:       DelayMode,
  pub(in crate::core) pending:    VecDeque<TimedPendingEntry<In>>,
  pub(in crate::core) tick_count: u64,
}

impl<In> TimedDelayLogic<In>
where
  In: Send + Sync + 'static,
{
  const fn ready_at(&self) -> u64 {
    match self.mode {
      | DelayMode::PerElement { delay_ticks } => self.tick_count.saturating_add(delay_ticks),
      | DelayMode::Initial { initial_delay_ticks } => {
        if self.tick_count < initial_delay_ticks {
          initial_delay_ticks
        } else {
          self.tick_count
        }
      },
    }
  }
}

impl<In> FlowLogic for TimedDelayLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let ready_at = self.ready_at();
    self.pending.push_back(TimedPendingEntry { ready_at, value });
    Ok(Vec::new())
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(entry) = self.pending.front() else {
      return Ok(Vec::new());
    };
    if entry.ready_at > self.tick_count {
      return Ok(Vec::new());
    }
    let Some(entry) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(entry.value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.tick_count = 0;
    Ok(())
  }
}
