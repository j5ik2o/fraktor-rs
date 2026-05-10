use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::{DownstreamCancelAction, SubstreamCancelStrategy};

#[cfg(test)]
mod tests;

pub(crate) struct GroupByLogic<In, Key, F> {
  pub(crate) max_substreams: usize,
  pub(crate) seen_keys: Vec<Key>,
  pub(crate) key_fn: F,
  pub(crate) substream_cancel_strategy: SubstreamCancelStrategy,
  pub(crate) source_done: bool,
  pub(crate) draining: bool,
  pub(crate) _pd: PhantomData<fn(In) -> Key>,
}

impl<In, Key, F> FlowLogic for GroupByLogic<In, Key, F>
where
  In: Send + Sync + 'static,
  Key: Clone + PartialEq + Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.max_substreams == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let key = (self.key_fn)(&value);
    if !self.seen_keys.contains(&key) {
      if self.seen_keys.len() >= self.max_substreams {
        return Err(StreamError::TooManySubstreamsOpen { max_substreams: self.max_substreams });
      }
      self.seen_keys.push(key.clone());
    }
    Ok(vec![Box::new((key, value)) as DynValue])
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

  fn wants_upstream_drain(&self) -> bool {
    self.draining && !self.source_done
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.seen_keys.clear();
    self.source_done = false;
    self.draining = false;
    Ok(())
  }
}
