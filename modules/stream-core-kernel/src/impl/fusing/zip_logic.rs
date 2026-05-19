use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct ZipLogic<In> {
  pub(crate) fan_in:      usize,
  pub(crate) edge_slots:  Vec<usize>,
  pub(crate) pending:     Vec<VecDeque<In>>,
  pub(crate) source_done: bool,
}

impl<In> ZipLogic<In>
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
    self.edge_slots.push(edge_index);
    self.pending.push(VecDeque::new());
    Ok(self.edge_slots.len().saturating_sub(1))
  }

  fn preferred_empty_input_slot(&self) -> Option<usize> {
    for (position, queue) in self.pending.iter().enumerate() {
      if queue.is_empty() {
        return self.edge_slots.get(position).copied();
      }
    }
    (0..self.fan_in).find(|slot| !self.edge_slots.contains(slot))
  }

  fn has_ready_group(&self) -> bool {
    self.pending.len() >= self.fan_in && self.pending.iter().all(|queue| !queue.is_empty())
  }

  fn pop_ready_group(&mut self) -> Option<Vec<In>> {
    if !self.has_ready_group() {
      return None;
    }
    let mut values = Vec::with_capacity(self.fan_in);
    for queue in &mut self.pending {
      let value = queue.pop_front()?;
      values.push(value);
    }
    Some(values)
  }
}

impl<In> FlowLogic for ZipLogic<In>
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

    if self.edge_slots.len() < self.fan_in {
      return Ok(Vec::new());
    }

    let Some(values) = self.pop_ready_group() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(values) as DynValue])
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn input_exhausted(&self, any_input_closed_empty: bool, all_inputs_closed_empty: bool) -> bool {
    let _ = all_inputs_closed_empty;
    any_input_closed_empty
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn can_accept_input_from_edge(&self, edge_slot: usize) -> bool {
    if self.source_done || edge_slot >= self.fan_in {
      return false;
    }
    let Some(position) = self.edge_slots.iter().position(|slot| *slot == edge_slot) else {
      return self.edge_slots.len() < self.fan_in;
    };
    self.pending.get(position).is_some_and(VecDeque::is_empty)
  }

  fn can_accept_input(&self) -> bool {
    !self.source_done && self.preferred_empty_input_slot().is_some()
  }

  fn preferred_input_edge_slot(&self) -> Option<usize> {
    self.preferred_empty_input_slot()
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(values) = self.pop_ready_group() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(values) as DynValue])
  }

  fn take_shutdown_request(&mut self) -> bool {
    self.source_done && !self.has_ready_group()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}
