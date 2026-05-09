use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct FilterLogic<In, F> {
  pub(crate) predicate: F,
  pub(crate) _pd:       PhantomData<fn(In)>,
}

impl<In, F> FlowLogic for FilterLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if (self.predicate)(&value) {
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(Vec::new())
  }
}
