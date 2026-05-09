use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct InterleaveLogic<In> {
  pub(crate) fan_in:      usize,
  pub(crate) edge_slots:  Vec<usize>,
  pub(crate) pending:     Vec<VecDeque<In>>,
  pub(crate) next_slot:   usize,
  pub(crate) source_done: bool,
}

impl<In> InterleaveLogic<In>
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
    if insert_at <= self.next_slot && self.edge_slots.len() > 1 {
      self.next_slot = self.next_slot.saturating_add(1) % self.edge_slots.len();
    }
    Ok(insert_at)
  }

  fn pop_next_value(&mut self) -> Option<In> {
    if self.pending.is_empty() {
      return None;
    }
    let start_slot = self.next_slot % self.pending.len();
    let mut slot = start_slot;
    for _ in 0..self.pending.len() {
      if let Some(value) = self.pending[slot].pop_front() {
        self.next_slot = (slot + 1) % self.pending.len();
        return Some(value);
      }
      slot = (slot + 1) % self.pending.len();
    }
    None
  }
}

impl<In> FlowLogic for InterleaveLogic<In>
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
    if let Some(next) = self.pop_next_value() {
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
    let Some(next) = self.pop_next_value() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(next) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.next_slot = 0;
    self.source_done = false;
    Ok(())
  }
}
