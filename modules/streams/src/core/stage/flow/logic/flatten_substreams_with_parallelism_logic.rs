use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct FlattenSubstreamsWithParallelismLogic<In> {
  pub(in crate::core::stage::flow) parallelism: usize,
  pub(in crate::core::stage::flow) _pd:         PhantomData<fn(In)>,
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
