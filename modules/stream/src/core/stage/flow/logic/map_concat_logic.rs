use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct MapConcatLogic<In, Out, F, I> {
  pub(in crate::core::stage::flow) func: F,
  pub(in crate::core::stage::flow) _pd:  PhantomData<fn(In) -> (Out, I)>,
}

impl<In, Out, F, I> FlowLogic for MapConcatLogic<In, Out, F, I>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = Out> + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.func)(value);
    Ok(output.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }
}
