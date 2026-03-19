use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct ZipWithIndexLogic<In> {
  pub(in crate::core::stage::flow) next_index: u64,
  pub(in crate::core::stage::flow) _pd:        PhantomData<fn(In)>,
}

impl<In> FlowLogic for ZipWithIndexLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let index = self.next_index;
    self.next_index = self.next_index.saturating_add(1);
    Ok(vec![Box::new((value, index)) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.next_index = 0;
    Ok(())
  }
}
