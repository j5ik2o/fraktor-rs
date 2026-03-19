use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct MapOptionLogic<In, Out, F> {
  pub(in crate::core::stage::flow) func: F,
  pub(in crate::core::stage::flow) _pd:  PhantomData<fn(In) -> Out>,
}

impl<In, Out, F> FlowLogic for MapOptionLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Option<Out> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let Some(output) = (self.func)(value) else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(output) as DynValue])
  }
}
