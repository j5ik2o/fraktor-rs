use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{
  DynValue, FlowLogic, Source, StreamError, StreamNotUsed, downcast_value,
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  stage_context::StageContext,
};

pub(in crate::core::stage::flow) struct FlatMapConcatLogic<In, Out, Mat2, F> {
  pub(in crate::core::stage::flow) func:          F,
  pub(in crate::core::stage::flow) active_inner:  Option<VecDeque<Out>>,
  pub(in crate::core::stage::flow) pending_outer: VecDeque<In>,
  pub(in crate::core::stage::flow) _pd:           PhantomData<fn(In) -> (Out, Mat2)>,
}

impl<In, Out, Mat2, F> FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn promote_outer_if_needed(&mut self) -> Result<(), StreamError> {
    while self.active_inner.is_none() {
      let Some(outer) = self.pending_outer.pop_front() else {
        return Ok(());
      };
      let source = (self.func)(outer);
      let outputs = source.collect_values()?;
      if outputs.is_empty() {
        continue;
      }
      let mut stream = VecDeque::with_capacity(outputs.len());
      stream.extend(outputs);
      self.active_inner = Some(stream);
    }
    Ok(())
  }

  fn pop_next_value(&mut self) -> Result<Option<Out>, StreamError> {
    self.promote_outer_if_needed()?;
    let Some(stream) = &mut self.active_inner else {
      return Ok(None);
    };
    let value = stream.pop_front();
    if stream.is_empty() {
      self.active_inner = None;
    }
    Ok(value)
  }
}

impl<In, Out, Mat2, F> FlowLogic for FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
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
