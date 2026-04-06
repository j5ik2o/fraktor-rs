use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core) struct ZipLogic<In> {
  pub(in crate::core) fan_in:     usize,
  pub(in crate::core) edge_slots: Vec<usize>,
  pub(in crate::core) pending:    Vec<VecDeque<In>>,
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

    let ready = self.pending.iter().all(|queue| !queue.is_empty());
    if !ready {
      return Ok(Vec::new());
    }

    let mut zipped = Vec::with_capacity(self.fan_in);
    for queue in &mut self.pending {
      let Some(item) = queue.pop_front() else {
        return Err(StreamError::InvalidConnection);
      };
      zipped.push(item);
    }

    Ok(vec![Box::new(zipped) as DynValue])
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    Ok(())
  }
}
