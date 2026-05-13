use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(crate) struct LimitWeightedLogic<In, FW>
where
  FW: FnMut(&In) -> usize + Send + Sync + 'static, {
  pub(crate) max_weight: u64,
  pub(crate) remaining:  usize,
  pub(crate) weight_fn:  FW,
  pub(crate) _pd:        PhantomData<fn(In)>,
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
      return Err(StreamError::StreamLimitReached { limit: self.max_weight });
    }
    self.remaining = self.remaining.saturating_sub(weight);
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    // `remaining` はこの stage インスタンス全体の上限として扱うため restart でも維持する。
    Ok(())
  }
}
