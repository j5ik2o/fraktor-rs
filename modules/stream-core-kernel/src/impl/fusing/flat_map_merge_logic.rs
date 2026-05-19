use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::marker::PhantomData;

use super::SecondarySourceBridge;
use crate::{
  DynValue, FlowLogic, StreamError, downcast_value,
  dsl::Source,
  materialization::StreamNotUsed,
  shape::{Inlet, Outlet, StreamShape},
  stage::{GraphStage, GraphStageLogic, StageContext},
};

pub(crate) struct FlatMapMergeLogic<In, Out, Mat2, F> {
  pub(crate) breadth:        usize,
  pub(crate) func:           F,
  pub(crate) active_streams: VecDeque<SecondarySourceBridge<Out>>,
  pub(crate) pending_outer:  VecDeque<In>,
  pub(crate) _pd:            PhantomData<fn(In) -> (Out, Mat2)>,
}

impl<In, Out, Mat2, F> FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn enqueue_active_inner(&mut self, value: In) -> Result<(), StreamError> {
    let stream = SecondarySourceBridge::new((self.func)(value))?;
    self.active_streams.push_back(stream);
    Ok(())
  }

  fn promote_pending(&mut self) -> Result<(), StreamError> {
    while self.active_streams.len() < self.breadth {
      let Some(value) = self.pending_outer.pop_front() else {
        break;
      };
      self.enqueue_active_inner(value)?;
    }
    Ok(())
  }

  fn pop_next_value(&mut self) -> Result<Option<Out>, StreamError> {
    self.promote_pending()?;
    loop {
      let active_len = self.active_streams.len();
      if active_len == 0 {
        return Ok(None);
      }

      let mut slot_freed = false;
      for _ in 0..active_len {
        let Some(mut stream) = self.active_streams.pop_front() else {
          break;
        };

        if let Some(value) = stream.poll_next()? {
          if stream.has_pending_output() {
            self.active_streams.push_back(stream);
          } else {
            slot_freed = true;
          }
          if slot_freed {
            self.promote_pending()?;
          }
          return Ok(Some(value));
        }

        if stream.has_pending_output() {
          self.active_streams.push_back(stream);
        } else {
          slot_freed = true;
        }
      }
      if slot_freed {
        self.promote_pending()?;
      }

      if self.active_streams.is_empty() {
        self.promote_pending()?;
        if self.active_streams.is_empty() {
          return Ok(None);
        }
        continue;
      }

      return Ok(None);
    }
  }
}

impl<In, Out, Mat2, F> FlowLogic for FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.breadth == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    self.pending_outer.push_back(value);
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    self.breadth > 0 && self.pending_outer.is_empty() && self.active_streams.len() < self.breadth
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    !self.active_streams.is_empty() || !self.pending_outer.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.active_streams.clear();
    self.pending_outer.clear();
    Ok(())
  }
}

impl<In, Out, Mat2, F> GraphStageLogic<In, Out, StreamNotUsed> for FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn on_push(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    let value = ctx.grab();
    self.pending_outer.push_back(value);
    match self.pop_next_value() {
      | Ok(Some(output)) => ctx.push(output),
      | Ok(None) => {},
      | Err(error) => ctx.fail(error),
    }
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

impl<In, Out, Mat2, F> GraphStage<In, Out, StreamNotUsed> for FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, Out> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, Out, StreamNotUsed> + Send> {
    Box::new(FlatMapMergeLogic {
      breadth:        self.breadth,
      func:           self.func.clone(),
      active_streams: VecDeque::new(),
      pending_outer:  VecDeque::new(),
      _pd:            PhantomData,
    })
  }
}
