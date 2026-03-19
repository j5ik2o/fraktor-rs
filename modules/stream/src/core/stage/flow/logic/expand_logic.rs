use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct ExpandLogic<In, F> {
  pub(in crate::core::stage::flow) expander:                F,
  pub(in crate::core::stage::flow) last:                    Option<In>,
  pub(in crate::core::stage::flow) pending: Option<core::iter::Peekable<Box<dyn Iterator<Item = In> + Send + 'static>>>,
  pub(in crate::core::stage::flow) tick_count:              u64,
  pub(in crate::core::stage::flow) last_input_tick:         Option<u64>,
  pub(in crate::core::stage::flow) last_extrapolation_tick: Option<u64>,
  pub(in crate::core::stage::flow) source_done:             bool,
}

impl<In, F, I> FlowLogic for ExpandLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = In> + 'static,
  <I as IntoIterator>::IntoIter: Send,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.last = Some(value);
    self.last_input_tick = Some(self.tick_count);
    self.last_extrapolation_tick = Some(self.tick_count);
    let Some(last) = self.last.as_ref() else {
      return Ok(Vec::new());
    };
    let mut iterator = (self.expander)(last).into_iter();
    if self.source_done {
      if let Some(next) = iterator.next() {
        return Ok(vec![Box::new(next) as DynValue]);
      }
      return Ok(Vec::new());
    }
    let iterator: Box<dyn Iterator<Item = In> + Send + 'static> = Box::new(iterator);
    self.pending = Some(iterator.peekable());
    self.drain_pending()
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    Ok(())
  }

  fn can_accept_input(&self) -> bool {
    true
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.source_done {
      self.pending = None;
      return Ok(Vec::new());
    }

    if let Some(iter) = &mut self.pending {
      if let Some(value) = iter.next() {
        if iter.peek().is_none() {
          self.pending = None;
        }
        return Ok(vec![Box::new(value) as DynValue]);
      }
      self.pending = None;
    }

    let Some(last_input_tick) = self.last_input_tick else {
      return Ok(Vec::new());
    };
    if self.tick_count <= last_input_tick || self.last_extrapolation_tick == Some(self.tick_count) {
      return Ok(Vec::new());
    }
    let Some(last) = self.last.as_ref() else {
      return Ok(Vec::new());
    };
    self.last_extrapolation_tick = Some(self.tick_count);
    let iterator: Box<dyn Iterator<Item = In> + Send + 'static> = Box::new((self.expander)(last).into_iter());
    self.pending = Some(iterator.peekable());

    if let Some(iter) = &mut self.pending {
      if let Some(value) = iter.next() {
        if iter.peek().is_none() {
          self.pending = None;
        }
        return Ok(vec![Box::new(value) as DynValue]);
      }
      self.pending = None;
    }
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    self.pending.is_some()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.last = None;
    self.pending = None;
    self.tick_count = 0;
    self.last_input_tick = None;
    self.last_extrapolation_tick = None;
    self.source_done = false;
    Ok(())
  }
}
