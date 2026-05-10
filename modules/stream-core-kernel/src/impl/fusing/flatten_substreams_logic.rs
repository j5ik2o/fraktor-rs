use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct FlattenSubstreamsLogic<In> {
  pub(crate) _pd: PhantomData<fn(In)>,
}

impl<In> FlowLogic for FlattenSubstreamsLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let values = downcast_value::<Vec<In>>(input)?;
    Ok(values.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }
}
