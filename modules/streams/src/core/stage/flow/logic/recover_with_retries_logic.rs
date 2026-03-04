use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct RecoverWithRetriesLogic<In> {
  pub(in crate::core::stage::flow) max_retries:  usize,
  pub(in crate::core::stage::flow) retries_left: usize,
  pub(in crate::core::stage::flow) fallback:     In,
}

impl<In> FlowLogic for RecoverWithRetriesLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Result<In, StreamError>>(input)?;
    match value {
      | Ok(value) => Ok(vec![Box::new(value) as DynValue]),
      | Err(_) => {
        if self.retries_left == 0 {
          return Err(StreamError::Failed);
        }
        self.retries_left = self.retries_left.saturating_sub(1);
        Ok(vec![Box::new(self.fallback.clone()) as DynValue])
      },
    }
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.retries_left = self.max_retries;
    Ok(())
  }
}
