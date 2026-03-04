use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct DropLogic<In> {
  pub(in crate::core::stage::flow) remaining: usize,
  pub(in crate::core::stage::flow) _pd:       PhantomData<fn(In)>,
}

impl<In> FlowLogic for DropLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.remaining > 0 {
      self.remaining = self.remaining.saturating_sub(1);
      return Ok(Vec::new());
    }
    Ok(vec![Box::new(value) as DynValue])
  }
}
