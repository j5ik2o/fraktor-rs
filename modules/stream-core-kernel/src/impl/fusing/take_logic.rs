use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct TakeLogic<In> {
  pub(crate) remaining: usize,
  pub(crate) _pd:       PhantomData<fn(In)>,
}

impl<In> FlowLogic for TakeLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.remaining == 0 {
      return Ok(Vec::new());
    }
    self.remaining = self.remaining.saturating_sub(1);
    Ok(vec![Box::new(value) as DynValue])
  }
}
