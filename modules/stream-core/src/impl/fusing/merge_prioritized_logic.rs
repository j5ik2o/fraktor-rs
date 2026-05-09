use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct MergePrioritizedLogic<In> {
  pub(crate) fan_in:      usize,
  pub(crate) priorities:  Vec<usize>,
  pub(crate) edge_slots:  Vec<usize>,
  pub(crate) pending:     Vec<VecDeque<In>>,
  pub(crate) credits:     Vec<usize>,
  pub(crate) current:     usize,
  pub(crate) source_done: bool,
}

impl<In> MergePrioritizedLogic<In>
where
  In: Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if edge_index >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    if !self.edge_slots.contains(&edge_index) {
      self.edge_slots.push(edge_index);
      self.edge_slots.sort_unstable();
    }
    if self.pending.len() < self.fan_in {
      self.pending.resize_with(self.fan_in, VecDeque::new);
    }
    if self.credits.len() < self.fan_in {
      self.credits = self.priorities.clone();
    }
    Ok(edge_index)
  }

  fn refill_credits(&mut self) {
    for (slot, credit) in self.credits.iter_mut().enumerate() {
      *credit = self.priorities[slot];
    }
  }

  pub(crate) fn pop_prioritized(&mut self) -> Option<In> {
    if self.fan_in == 0 || self.pending.is_empty() {
      return None;
    }
    if self.credits.len() < self.fan_in {
      self.refill_credits();
    }
    // 加重ラウンドロビン: 現在のスロットからクレジットに基づいて要素を取得
    for _ in 0..self.fan_in {
      let slot = self.current % self.fan_in;
      if self.credits[slot] > 0
        && let Some(value) = self.pending[slot].pop_front()
      {
        self.credits[slot] = self.credits[slot].saturating_sub(1);
        if self.credits[slot] == 0 {
          self.current = (slot + 1) % self.fan_in;
        }
        return Some(value);
      }
      self.current = (slot + 1) % self.fan_in;
    }
    // 全クレジット消費済み → 再充填して再試行
    self.refill_credits();
    for _ in 0..self.fan_in {
      let slot = self.current % self.fan_in;
      if let Some(value) = self.pending[slot].pop_front() {
        self.credits[slot] = self.credits[slot].saturating_sub(1);
        if self.credits[slot] == 0 {
          self.current = (slot + 1) % self.fan_in;
        }
        return Some(value);
      }
      self.current = (slot + 1) % self.fan_in;
    }
    None
  }
}

impl<In> FlowLogic for MergePrioritizedLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);
    if let Some(next) = self.pop_prioritized() {
      return Ok(vec![Box::new(next) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn preferred_input_edge_slot(&self) -> Option<usize> {
    if self.fan_in == 0 {
      return None;
    }
    if self.credits.is_empty() {
      return Some(self.current % self.fan_in);
    }
    for offset in 0..self.fan_in {
      let slot = (self.current + offset) % self.fan_in;
      if self.credits[slot] > 0 {
        return Some(slot);
      }
    }
    Some(self.current % self.fan_in)
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(next) = self.pop_prioritized() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(next) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.credits.clear();
    self.current = 0;
    self.source_done = false;
    Ok(())
  }
}
