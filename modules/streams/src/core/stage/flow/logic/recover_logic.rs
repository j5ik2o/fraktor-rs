use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct RecoverLogic<In> {
  pub(in crate::core::stage::flow) fallback: In,
}

impl<In> FlowLogic for RecoverLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Result<In, StreamError>>(input)?;
    match value {
      | Ok(value) => Ok(vec![Box::new(value) as DynValue]),
      | Err(_) => Ok(vec![Box::new(self.fallback.clone()) as DynValue]),
    }
  }
}
