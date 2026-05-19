use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct StatefulMapConcatLogic<In, Out, Factory, Mapper, I> {
  pub(crate) factory: Factory,
  pub(crate) mapper:  Mapper,
  pub(crate) _pd:     PhantomData<fn(In) -> (Out, I)>,
}

impl<In, Out, Factory, Mapper, I> FlowLogic for StatefulMapConcatLogic<In, Out, Factory, Mapper, I>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Factory: FnMut() -> Mapper + Send + Sync + 'static,
  Mapper: FnMut(In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = Out> + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.mapper)(value);
    Ok(output.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.mapper = (self.factory)();
    Ok(())
  }
}
