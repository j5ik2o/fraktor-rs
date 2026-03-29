use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use crate::core::{
  DynValue, FlowLogic, StreamError, StreamNotUsed, downcast_value,
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  stage::StageContext,
};

pub(in crate::core) struct MapLogic<In, Out, F> {
  pub(in crate::core) func: F,
  pub(in crate::core) _pd:  PhantomData<fn(In) -> Out>,
}

impl<In, Out, F> FlowLogic for MapLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + 'static,
  F: FnMut(In) -> Out + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.func)(value);
    Ok(vec![Box::new(output)])
  }
}

impl<In, Out, F> GraphStageLogic<In, Out, StreamNotUsed> for MapLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Out + Send + Sync + 'static,
{
  fn on_push(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    let value = ctx.grab();
    let output = (self.func)(value);
    ctx.push(output);
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

impl<In, Out, F> GraphStage<In, Out, StreamNotUsed> for MapLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Out + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, Out> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, Out, StreamNotUsed> + Send> {
    Box::new(MapLogic { func: self.func.clone(), _pd: PhantomData })
  }
}
