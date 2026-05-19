use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct MergeSortedLogic<In> {
  pub(crate) fan_in:      usize,
  pub(crate) edge_slots:  Vec<usize>,
  pub(crate) pending:     Vec<VecDeque<In>>,
  pub(crate) source_done: bool,
}

impl<In> MergeSortedLogic<In>
where
  In: Ord + Send + Sync + 'static,
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

  fn pop_sorted(&mut self) -> Option<In> {
    if self.pending.is_empty() {
      return None;
    }
    // source_done前は全fan_inスロットが登録され、かつ全てに要素が揃うのを待つ
    if !self.source_done {
      if self.pending.len() < self.fan_in {
        return None;
      }
      let all_have_data = self.pending.iter().all(|queue| !queue.is_empty());
      if !all_have_data {
        return None;
      }
    }
    // 全スロットの先頭要素を比較して最小値のスロットを選択
    let mut min_slot: Option<usize> = None;
    for (slot, queue) in self.pending.iter().enumerate() {
      if let Some(front) = queue.front() {
        match min_slot {
          | None => min_slot = Some(slot),
          | Some(current_min) => {
            if let Some(current_front) = self.pending[current_min].front()
              && front < current_front
            {
              min_slot = Some(slot);
            }
          },
        }
      }
    }
    min_slot.and_then(|slot| self.pending[slot].pop_front())
  }
}

impl<In> FlowLogic for MergeSortedLogic<In>
where
  In: Ord + Send + Sync + 'static,
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
    if let Some(next) = self.pop_sorted() {
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
    let Some(next) = self.pop_sorted() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(next) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}
