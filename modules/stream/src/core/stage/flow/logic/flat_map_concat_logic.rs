use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::marker::PhantomData;

use super::{
  super::super::{
    DynValue, FlowLogic, Source, StreamError, StreamNotUsed, downcast_value,
    graph::{GraphStage, GraphStageLogic},
    shape::{Inlet, Outlet, StreamShape},
    stage_context::StageContext,
  },
  SecondarySourceBridge,
};
use crate::core::DownstreamCancelAction;

pub(in crate::core::stage::flow) struct FlatMapConcatLogic<In, Out, Mat2, F> {
  pub(in crate::core::stage::flow) func:          F,
  pub(in crate::core::stage::flow) active_inner:  Option<SecondarySourceBridge<Out>>,
  pub(in crate::core::stage::flow) pending_outer: VecDeque<In>,
  pub(in crate::core::stage::flow) _pd:           PhantomData<fn(In) -> (Out, Mat2)>,
}

impl<In, Out, Mat2, F> FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn promote_outer_if_needed(&mut self) -> Result<(), StreamError> {
    while self.active_inner.is_none() {
      let Some(outer) = self.pending_outer.pop_front() else {
        return Ok(());
      };
      let inner = SecondarySourceBridge::new((self.func)(outer))?;
      self.active_inner = Some(inner);
    }
    Ok(())
  }

  fn pop_next_value(&mut self) -> Result<Option<Out>, StreamError> {
    loop {
      self.promote_outer_if_needed()?;
      let Some(stream) = self.active_inner.as_mut() else {
        return Ok(None);
      };
      if let Some(value) = stream.poll_next()? {
        if !stream.has_pending_output() {
          self.active_inner = None;
        }
        return Ok(Some(value));
      }
      if stream.has_pending_output() {
        return Ok(None);
      }
      self.active_inner = None;
    }
  }
}

impl<In, Out, Mat2, F> FlowLogic for FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.pending_outer.push_back(value);
    self.drain_pending()
  }

  fn can_accept_input(&self) -> bool {
    self.active_inner.is_none() && self.pending_outer.is_empty()
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    self.active_inner = None;
    self.pending_outer.clear();
    Ok(DownstreamCancelAction::Propagate)
  }

  fn has_pending_output(&self) -> bool {
    !self.pending_outer.is_empty() || self.active_inner.as_ref().is_some_and(SecondarySourceBridge::has_pending_output)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.active_inner = None;
    self.pending_outer.clear();
    Ok(())
  }
}

impl<In, Out, Mat2, F> GraphStageLogic<In, Out, StreamNotUsed> for FlatMapConcatLogic<In, Out, Mat2, F>
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

impl<In, Out, Mat2, F> GraphStage<In, Out, StreamNotUsed> for FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, Out> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, Out, StreamNotUsed>> {
    Box::new(FlatMapConcatLogic {
      func:          self.func.clone(),
      active_inner:  None,
      pending_outer: VecDeque::new(),
      _pd:           PhantomData,
    })
  }
}
