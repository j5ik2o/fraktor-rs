use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct ConflateWithSeedLogic<In, T, FS, FA> {
  pub(in crate::core::stage::flow) seed:         FS,
  pub(in crate::core::stage::flow) aggregate:    FA,
  pub(in crate::core::stage::flow) pending:      Option<T>,
  pub(in crate::core::stage::flow) just_updated: bool,
  pub(in crate::core::stage::flow) _pd:          core::marker::PhantomData<fn(In) -> T>,
}

impl<In, T, FS, FA> FlowLogic for ConflateWithSeedLogic<In, T, FS, FA>
where
  In: Send + Sync + 'static,
  T: Send + Sync + 'static,
  FS: FnMut(In) -> T + Send + Sync + 'static,
  FA: FnMut(T, In) -> T + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let aggregated =
      if let Some(current) = self.pending.take() { (self.aggregate)(current, value) } else { (self.seed)(value) };
    self.pending = Some(aggregated);
    self.just_updated = true;
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    true
  }

  fn can_accept_input_while_output_buffered(&self) -> bool {
    true
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(aggregated) = self.pending.take() else {
      return Ok(Vec::new());
    };

    if self.just_updated {
      self.pending = Some(aggregated);
      self.just_updated = false;
      return Ok(Vec::new());
    }

    Ok(vec![Box::new(aggregated) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    self.pending.is_some()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending = None;
    self.just_updated = false;
    Ok(())
  }
}
