use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct FlattenSubstreamsWithParallelismLogic<In> {
  pub(crate) parallelism: usize,
  pub(crate) _pd:         PhantomData<fn(In)>,
}

impl<In> FlowLogic for FlattenSubstreamsWithParallelismLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.parallelism == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let values = downcast_value::<Vec<In>>(input)?;
    Ok(values.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }
}
