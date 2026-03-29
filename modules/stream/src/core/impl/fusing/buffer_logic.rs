use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::core::buffer::OverflowStrategy;

pub(in crate::core) struct BufferLogic<In> {
  pub(in crate::core) capacity:          usize,
  pub(in crate::core) overflow_strategy: OverflowStrategy,
  pub(in crate::core) pending:           VecDeque<In>,
  pub(in crate::core) source_done:       bool,
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

    match self.overflow_strategy {
      | OverflowStrategy::Backpressure => Ok(()),
      | OverflowStrategy::DropHead => {
        let _ = self.pending.pop_front();
        self.pending.push_back(value);
        Ok(())
      },
      | OverflowStrategy::DropTail => {
        let _ = self.pending.pop_back();
        self.pending.push_back(value);
        Ok(())
      },
      | OverflowStrategy::DropBuffer => {
        self.pending.clear();
        self.pending.push_back(value);
        Ok(())
      },
      | OverflowStrategy::Fail => Err(StreamError::BufferOverflow),
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

  fn can_accept_input(&self) -> bool {
    match self.overflow_strategy {
      | OverflowStrategy::Backpressure => self.pending.len() < self.capacity,
      | _ => true,
    }
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let can_emit_on_backpressure =
      matches!(self.overflow_strategy, OverflowStrategy::Backpressure) && self.pending.len() >= self.capacity;
    if !self.source_done && !can_emit_on_backpressure {
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
