use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct TakeUntilLogic<In, F> {
  pub(in crate::core::stage::flow) predicate:          F,
  pub(in crate::core::stage::flow) taking:             bool,
  pub(in crate::core::stage::flow) shutdown_requested: bool,
  pub(in crate::core::stage::flow) _pd:                PhantomData<fn(In)>,
}

impl<In, F> FlowLogic for TakeUntilLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if !self.taking {
      return Ok(Vec::new());
    }
    if (self.predicate)(&value) {
      self.taking = false;
      self.shutdown_requested = true;
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(vec![Box::new(value) as DynValue])
  }

  fn take_shutdown_request(&mut self) -> bool {
    let requested = self.shutdown_requested;
    self.shutdown_requested = false;
    requested
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.taking = true;
    self.shutdown_requested = false;
    Ok(())
  }
}
