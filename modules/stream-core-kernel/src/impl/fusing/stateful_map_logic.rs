use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct StatefulMapLogic<In, Out, Factory, Mapper> {
  pub(crate) factory: Factory,
  pub(crate) mapper:  Mapper,
  pub(crate) _pd:     PhantomData<fn(In) -> Out>,
}

impl<In, Out, Factory, Mapper> FlowLogic for StatefulMapLogic<In, Out, Factory, Mapper>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Factory: FnMut() -> Mapper + Send + Sync + 'static,
  Mapper: FnMut(In) -> Out + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.mapper)(value);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.mapper = (self.factory)();
    Ok(())
  }
}
