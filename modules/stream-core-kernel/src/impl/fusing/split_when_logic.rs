use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::{DownstreamCancelAction, SubstreamCancelStrategy};

pub(crate) struct SplitWhenLogic<In, F> {
  pub(crate) predicate:                 F,
  pub(crate) substream_cancel_strategy: SubstreamCancelStrategy,
  pub(crate) current:                   Vec<In>,
  pub(crate) source_done:               bool,
  pub(crate) draining:                  bool,
}

impl<In, F> FlowLogic for SplitWhenLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let should_split = (self.predicate)(&value);
    if should_split && !self.current.is_empty() {
      let output = core::mem::take(&mut self.current);
      self.current.push(value);
      return Ok(vec![Box::new(output) as DynValue]);
    }
    self.current.push(value);
    Ok(Vec::new())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    match self.substream_cancel_strategy {
      | SubstreamCancelStrategy::Drain => {
        self.draining = true;
        Ok(DownstreamCancelAction::Drain)
      },
      | SubstreamCancelStrategy::Propagate => self.on_source_done().map(|()| DownstreamCancelAction::Propagate),
    }
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done || self.current.is_empty() {
      return Ok(Vec::new());
    }
    let output = core::mem::take(&mut self.current);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.current.clear();
    self.source_done = false;
    self.draining = false;
    Ok(())
  }

  fn wants_upstream_drain(&self) -> bool {
    self.draining && !self.source_done
  }
}
