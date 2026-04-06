use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core) struct IntersperseLogic<In> {
  pub(in crate::core) start:       In,
  pub(in crate::core) inject:      In,
  pub(in crate::core) end:         In,
  pub(in crate::core) pending:     VecDeque<In>,
  pub(in crate::core) needs_start: bool,
  pub(in crate::core) seen_value:  bool,
  pub(in crate::core) source_done: bool,
  pub(in crate::core) end_emitted: bool,
}

impl<In> FlowLogic for IntersperseLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.needs_start {
      self.pending.push_back(self.start.clone());
      self.needs_start = false;
    }
    if self.seen_value {
      self.pending.push_back(self.inject.clone());
    }
    self.pending.push_back(value);
    self.seen_value = true;
    self.drain_pending()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.source_done {
      if self.needs_start {
        self.pending.push_back(self.start.clone());
        self.needs_start = false;
      }
      if !self.end_emitted {
        self.pending.push_back(self.end.clone());
        self.end_emitted = true;
      }
    }
    Ok(self.pending.drain(..).map(|value| Box::new(value) as DynValue).collect())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.needs_start = true;
    self.seen_value = false;
    self.source_done = false;
    self.end_emitted = false;
    Ok(())
  }
}
