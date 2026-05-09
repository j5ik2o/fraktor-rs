use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct TakeUntilLogic<In, F> {
  pub(crate) predicate:          F,
  pub(crate) taking:             bool,
  pub(crate) shutdown_requested: bool,
  pub(crate) _pd:                PhantomData<fn(In)>,
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
