use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct SlidingLogic<In> {
  pub(crate) size:   usize,
  pub(crate) window: VecDeque<In>,
}

impl<In> FlowLogic for SlidingLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.size == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    self.window.push_back(value);
    if self.window.len() < self.size {
      return Ok(Vec::new());
    }
    if self.window.len() > self.size {
      let _ = self.window.pop_front();
    }
    let output = self.window.iter().cloned().collect::<Vec<In>>();
    let _ = self.window.pop_front();
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.window.clear();
    Ok(())
  }
}
