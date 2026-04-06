use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core) struct GroupedWeightedWithinLogic<In, FW>
where
  FW: FnMut(&In) -> usize + Send + Sync + 'static, {
  pub(in crate::core) max_weight:        usize,
  pub(in crate::core) duration_ticks:    Option<u64>,
  pub(in crate::core) tick_count:        u64,
  pub(in crate::core) window_start_tick: Option<u64>,
  pub(in crate::core) current:           Vec<In>,
  pub(in crate::core) current_weight:    usize,
  pub(in crate::core) pending:           VecDeque<Vec<In>>,
  pub(in crate::core) weight_fn:         FW,
}

impl<In, FW> GroupedWeightedWithinLogic<In, FW>
where
  In: Send + Sync + 'static,
  FW: FnMut(&In) -> usize + Send + Sync + 'static,
{
  fn tick_window_expired(&self) -> bool {
    self.duration_ticks.is_some_and(|duration_ticks| {
      self
        .window_start_tick
        .is_some_and(|window_start_tick| self.tick_count >= window_start_tick.saturating_add(duration_ticks))
    })
  }

  fn flush_current(&mut self) {
    if self.current.is_empty() {
      return;
    }
    self.pending.push_back(core::mem::take(&mut self.current));
    self.current_weight = 0;
    self.window_start_tick = None;
  }
}

impl<In, FW> FlowLogic for GroupedWeightedWithinLogic<In, FW>
where
  In: Send + Sync + 'static,
  FW: FnMut(&In) -> usize + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let weight = (self.weight_fn)(&value);
    let would_exceed_weight =
      self.current_weight.checked_add(weight).is_none_or(|next_weight| next_weight > self.max_weight);

    if !self.current.is_empty() && would_exceed_weight {
      self.flush_current();
    }

    if self.current.is_empty() {
      self.window_start_tick = Some(self.tick_count);
    }

    self.current_weight = self.current_weight.saturating_add(weight);
    self.current.push(value);

    if self.current_weight >= self.max_weight {
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
    self.current_weight = 0;
    self.pending.clear();
    Ok(())
  }
}
