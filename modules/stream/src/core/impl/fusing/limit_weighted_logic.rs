use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core) struct LimitWeightedLogic<In, FW>
where
  FW: FnMut(&In) -> usize + Send + Sync + 'static, {
  pub(in crate::core) remaining:          usize,
  pub(in crate::core) weight_fn:          FW,
  pub(in crate::core) shutdown_requested: bool,
  pub(in crate::core) _pd:                PhantomData<fn(In)>,
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
      self.shutdown_requested = true;
      return Ok(Vec::new());
    }
    self.remaining = self.remaining.saturating_sub(weight);
    Ok(vec![Box::new(value) as DynValue])
  }

  fn take_shutdown_request(&mut self) -> bool {
    let requested = self.shutdown_requested;
    self.shutdown_requested = false;
    requested
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    // `remaining` はこの stage インスタンス全体の上限として扱うため restart でも維持する。
    self.shutdown_requested = false;
    Ok(())
  }
}
