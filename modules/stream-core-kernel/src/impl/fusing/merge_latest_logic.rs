use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct MergeLatestLogic<In> {
  pub(crate) fan_in:     usize,
  pub(crate) edge_slots: Vec<usize>,
  pub(crate) latest:     Vec<Option<In>>,
  pub(crate) all_seen:   bool,
}

impl<In> MergeLatestLogic<In>
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
    self.latest.insert(insert_at, None);
    Ok(insert_at)
  }

  fn try_emit(&self) -> Option<Vec<In>> {
    if !self.all_seen {
      return None;
    }
    Some(self.latest.iter().filter_map(|opt| opt.as_ref().cloned()).collect())
  }
}

impl<In> FlowLogic for MergeLatestLogic<In>
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
    self.latest[slot] = Some(value);
    // 全スロットが一度でもSomeになったかチェック
    if !self.all_seen && self.latest.len() >= self.fan_in && self.latest.iter().all(|opt| opt.is_some()) {
      self.all_seen = true;
    }
    if let Some(values) = self.try_emit() {
      return Ok(vec![Box::new(values) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.latest.clear();
    self.all_seen = false;
    Ok(())
  }
}
