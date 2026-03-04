use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct BufferLogic<In> {
  pub(in crate::core::stage::flow) capacity:        usize,
  pub(in crate::core::stage::flow) overflow_policy: OverflowPolicy,
  pub(in crate::core::stage::flow) pending:         VecDeque<In>,
  pub(in crate::core::stage::flow) source_done:     bool,
}

impl<In> BufferLogic<In>
where
  In: Send + Sync + 'static,
{
  fn offer_with_strategy(&mut self, value: In) -> Result<(), StreamError> {
    if self.capacity == 0 {
      return Err(StreamError::InvalidConnection);
    }
    if self.pending.len() < self.capacity {
      self.pending.push_back(value);
      return Ok(());
    }

    match self.overflow_policy {
      | OverflowPolicy::Block => Err(StreamError::BufferOverflow),
      | OverflowPolicy::DropNewest => Ok(()),
      | OverflowPolicy::DropOldest => {
        let _ = self.pending.pop_front();
        self.pending.push_back(value);
        Ok(())
      },
      | OverflowPolicy::Grow => {
        self.pending.push_back(value);
        Ok(())
      },
    }
  }
}

impl<In> FlowLogic for BufferLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.offer_with_strategy(value)?;
    Ok(Vec::new())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(value) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}
