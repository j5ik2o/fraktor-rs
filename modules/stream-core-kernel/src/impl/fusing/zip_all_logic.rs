use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct ZipAllLogic<In> {
  pub(crate) fan_in:      usize,
  pub(crate) fill_value:  In,
  pub(crate) edge_slots:  Vec<usize>,
  pub(crate) pending:     Vec<VecDeque<In>>,
  pub(crate) source_done: bool,
}

impl<In> ZipAllLogic<In>
where
  In: Clone + Send + Sync + 'static,
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
    Ok(insert_at)
  }

  fn pop_ready_group(&mut self) -> Option<Vec<In>> {
    if self.pending.len() < self.fan_in {
      return None;
    }
    let ready = self.pending.iter().all(|queue| !queue.is_empty());
    if !ready {
      return None;
    }
    let mut values = Vec::with_capacity(self.fan_in);
    for queue in &mut self.pending {
      let value = queue.pop_front()?;
      values.push(value);
    }
    Some(values)
  }

  fn pop_with_fill_after_completion(&mut self) -> Option<Vec<In>> {
    if self.pending.iter().all(|queue| queue.is_empty()) {
      return None;
    }
    let mut values = Vec::with_capacity(self.fan_in);
    for queue in &mut self.pending {
      if let Some(value) = queue.pop_front() {
        values.push(value);
      } else {
        values.push(self.fill_value.clone());
      }
    }
    for _ in self.pending.len()..self.fan_in {
      values.push(self.fill_value.clone());
    }
    Some(values)
  }
}

impl<In> FlowLogic for ZipAllLogic<In>
where
  In: Clone + Send + Sync + 'static,
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

    if let Some(values) = self.pop_ready_group() {
      return Ok(vec![Box::new(values) as DynValue]);
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
    if let Some(values) = self.pop_ready_group() {
      return Ok(vec![Box::new(values) as DynValue]);
    }
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(values) = self.pop_with_fill_after_completion() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(values) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}
