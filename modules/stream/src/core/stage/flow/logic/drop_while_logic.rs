use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct DropWhileLogic<In, F> {
  pub(in crate::core::stage::flow) predicate: F,
  pub(in crate::core::stage::flow) dropping:  bool,
  pub(in crate::core::stage::flow) _pd:       PhantomData<fn(In)>,
}

impl<In, F> FlowLogic for DropWhileLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.dropping && (self.predicate)(&value) {
      return Ok(Vec::new());
    }
    self.dropping = false;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.dropping = true;
    Ok(())
  }
}
