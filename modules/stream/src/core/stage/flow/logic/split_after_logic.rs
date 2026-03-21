use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::core::{DownstreamCancelAction, SubstreamCancelStrategy};

pub(in crate::core::stage::flow) struct SplitAfterLogic<In, F> {
  pub(in crate::core::stage::flow) predicate:                 F,
  pub(in crate::core::stage::flow) substream_cancel_strategy: SubstreamCancelStrategy,
  pub(in crate::core::stage::flow) current:                   Vec<In>,
  pub(in crate::core::stage::flow) source_done:               bool,
  pub(in crate::core::stage::flow) draining:                  bool,
}

impl<In, F> FlowLogic for SplitAfterLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let should_split = (self.predicate)(&value);
    self.current.push(value);
    if !should_split {
      return Ok(Vec::new());
    }
    let output = core::mem::take(&mut self.current);
    Ok(vec![Box::new(output) as DynValue])
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
