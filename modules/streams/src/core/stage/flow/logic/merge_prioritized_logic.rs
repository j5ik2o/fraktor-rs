use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct MergePrioritizedLogic<In> {
  pub(in crate::core::stage::flow) fan_in:      usize,
  pub(in crate::core::stage::flow) priorities:  Vec<usize>,
  pub(in crate::core::stage::flow) edge_slots:  Vec<usize>,
  pub(in crate::core::stage::flow) pending:     Vec<VecDeque<In>>,
  pub(in crate::core::stage::flow) credits:     Vec<usize>,
  pub(in crate::core::stage::flow) current:     usize,
  pub(in crate::core::stage::flow) source_done: bool,
}

impl<In> MergePrioritizedLogic<In>
where
  In: Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.pending.insert(insert_at, VecDeque::new());
    // 仮クレジットを挿入後、全スロットのクレジットを再計算する。
    // 挿入によりスロットがシフトするため、個別設定だとpriorities[slot]との不整合が発生する。
    self.credits.insert(insert_at, 0);
    self.refill_credits();
    if insert_at <= self.current && self.edge_slots.len() > 1 {
      self.current = self.current.saturating_add(1) % self.edge_slots.len();
    }
    Ok(insert_at)
  }

  fn refill_credits(&mut self) {
    for (slot, credit) in self.credits.iter_mut().enumerate() {
      *credit = self.priorities[slot];
    }
  }

  pub(in crate::core::stage::flow) fn pop_prioritized(&mut self) -> Option<In> {
    if self.pending.is_empty() {
      return None;
    }
    let len = self.pending.len();
    // 加重ラウンドロビン: 現在のスロットからクレジットに基づいて要素を取得
    for _ in 0..len {
      let slot = self.current % len;
      if self.credits[slot] > 0
        && let Some(value) = self.pending[slot].pop_front()
      {
        self.credits[slot] = self.credits[slot].saturating_sub(1);
        if self.credits[slot] == 0 {
          self.current = (slot + 1) % len;
        }
        return Some(value);
      }
      self.current = (slot + 1) % len;
    }
    // 全クレジット消費済み → 再充填して再試行
    self.refill_credits();
    for _ in 0..len {
      let slot = self.current % len;
      if let Some(value) = self.pending[slot].pop_front() {
        self.credits[slot] = self.credits[slot].saturating_sub(1);
        if self.credits[slot] == 0 {
          self.current = (slot + 1) % len;
        }
        return Some(value);
      }
      self.current = (slot + 1) % len;
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
