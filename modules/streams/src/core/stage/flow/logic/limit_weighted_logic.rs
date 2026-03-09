use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct LimitWeightedLogic<In, FW>
where
  FW: FnMut(&In) -> usize + Send + Sync + 'static, {
  pub(in crate::core::stage::flow) remaining: usize,
  pub(in crate::core::stage::flow) weight_fn: FW,
  pub(in crate::core::stage::flow) _pd:       PhantomData<fn(In)>,
}

impl<In, FW> FlowLogic for LimitWeightedLogic<In, FW>
where
  In: Send + Sync + 'static,
  FW: FnMut(&In) -> usize + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let weight = (self.weight_fn)(&value);
    if self.remaining == 0 || weight > self.remaining {
      self.remaining = 0;
      return Ok(Vec::new());
    }
    self.remaining = self.remaining.saturating_sub(weight);
    Ok(vec![Box::new(value) as DynValue])
  }
}
