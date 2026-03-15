use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::{
  super::super::super::{DynValue, FlowLogic, StreamError, delay_strategy::DelayStrategy, downcast_value},
  timed_delay_logic::TimedPendingEntry,
};

/// Flow logic that delays each element using a [`DelayStrategy`].
///
/// Unlike [`TimedDelayLogic`](super::timed_delay_logic::TimedDelayLogic)
/// which applies a fixed or initial delay, this logic delegates the
/// per-element delay decision to a caller-supplied strategy.
pub(in crate::core::stage::flow) struct StrategyDelayLogic<In, S> {
  strategy:   S,
  pending:    VecDeque<TimedPendingEntry<In>>,
  tick_count: u64,
}

impl<In, S> StrategyDelayLogic<In, S>
where
  S: DelayStrategy<In>,
{
  pub(in crate::core::stage::flow) const fn new(strategy: S) -> Self {
    Self { strategy, pending: VecDeque::new(), tick_count: 0 }
  }
}

impl<In, S> FlowLogic for StrategyDelayLogic<In, S>
where
  In: Send + Sync + 'static,
  S: DelayStrategy<In> + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let delay_ticks = self.strategy.next_delay(&value);
    let ready_at = self.tick_count.saturating_add(delay_ticks);
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
