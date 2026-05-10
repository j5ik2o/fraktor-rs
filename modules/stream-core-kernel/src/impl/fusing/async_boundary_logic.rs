use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct AsyncBoundaryLogic<In> {
  pub(crate) pending:   VecDeque<In>,
  pub(crate) capacity:  usize,
  pub(crate) enforcing: bool,
}

impl<In> FlowLogic for AsyncBoundaryLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.enforcing && self.pending.len() >= self.capacity {
      return Err(StreamError::BufferOverflow);
    }
    let value = downcast_value::<In>(input)?;
    self.pending.push_back(value);
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    // Enforcing mode accepts input unconditionally so apply() can detect
    // capacity overflow and fail with BufferOverflow (matching Pekko's
    // RateExceededException semantics). Shaping mode uses backpressure.
    self.enforcing || self.pending.len() < self.capacity
  }

  fn can_accept_input_while_output_buffered(&self) -> bool {
    // Enforcing mode does not backpressure — it accepts input even when
    // downstream is slow, and fails on overflow (Pekko RateExceededException).
    self.enforcing
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(value) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    Ok(())
  }
}
