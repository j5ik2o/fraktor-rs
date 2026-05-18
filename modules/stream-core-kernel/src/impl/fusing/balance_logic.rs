use alloc::{vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError};

pub(crate) struct BalanceLogic<In> {
  pub(crate) fan_out: usize,
  pub(crate) _pd:     PhantomData<fn(In)>,
}

impl<In> FlowLogic for BalanceLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_out == 0 {
      return Err(StreamError::InvalidConnection);
    }
    Ok(vec![input])
  }

  fn expected_fan_out(&self) -> Option<usize> {
    Some(self.fan_out)
  }
}
