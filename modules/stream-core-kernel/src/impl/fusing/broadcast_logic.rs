use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct BroadcastLogic<In> {
  pub(crate) fan_out: usize,
  pub(crate) _pd:     PhantomData<fn(In)>,
}

impl<In> FlowLogic for BroadcastLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_out == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let mut outputs = Vec::with_capacity(self.fan_out);
    for _ in 0..self.fan_out {
      outputs.push(Box::new(value.clone()) as DynValue);
    }
    Ok(outputs)
  }

  fn expected_fan_out(&self) -> Option<usize> {
    Some(self.fan_out)
  }
}
