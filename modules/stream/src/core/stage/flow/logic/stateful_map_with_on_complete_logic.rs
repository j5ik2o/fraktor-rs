use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct StatefulMapWithOnCompleteLogic<In, Out, S, Factory, Mapper, OnComplete> {
  pub(in crate::core::stage::flow) factory:     Factory,
  pub(in crate::core::stage::flow) state:       S,
  pub(in crate::core::stage::flow) mapper:      Mapper,
  pub(in crate::core::stage::flow) on_complete: OnComplete,
  pub(in crate::core::stage::flow) source_done: bool,
  // on_complete から最大 1 件の終端要素が生成されるため Option で表現する。
  pub(in crate::core::stage::flow) pending:     Option<DynValue>,
  pub(in crate::core::stage::flow) _pd:         PhantomData<fn(In) -> Out>,
}

impl<In, Out, S, Factory, Mapper, OnComplete> FlowLogic
  for StatefulMapWithOnCompleteLogic<In, Out, S, Factory, Mapper, OnComplete>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  S: Send + Sync + 'static,
  Factory: FnMut() -> S + Send + Sync + 'static,
  Mapper: FnMut(&mut S, In) -> Out + Send + Sync + 'static,
  OnComplete: FnMut(S) -> Option<Out> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.mapper)(&mut self.state, value);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    let state = core::mem::replace(&mut self.state, (self.factory)());
    if let Some(final_value) = (self.on_complete)(state) {
      self.pending = Some(Box::new(final_value) as DynValue);
    }
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(match self.pending.take() {
      | Some(value) => vec![value],
      | None => Vec::new(),
    })
  }

  fn has_pending_output(&self) -> bool {
    self.pending.is_some()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.state = (self.factory)();
    self.source_done = false;
    self.pending = None;
    Ok(())
  }
}
