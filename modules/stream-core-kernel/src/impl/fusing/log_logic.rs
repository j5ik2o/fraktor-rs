use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::FailureAction;

#[cfg(test)]
mod tests;

pub(crate) struct LogLogic<In> {
  pub(crate) _pd: PhantomData<fn(In)>,
}

impl<In> LogLogic<In> {
  pub(crate) fn new() -> Self {
    Self { _pd: PhantomData }
  }
}

impl<In> FlowLogic for LogLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn handles_failures(&self) -> bool {
    false
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    Ok(FailureAction::Propagate(error))
  }
}
