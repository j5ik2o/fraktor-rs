#![cfg(feature = "compression")]

use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

pub(in crate::core::stage::flow) struct TryMapConcatLogic<In, Out, F> {
  pub(in crate::core::stage::flow) func: F,
  pub(in crate::core::stage::flow) _pd:  PhantomData<fn(In) -> Out>,
}

impl<In, Out, F> FlowLogic for TryMapConcatLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Result<Vec<Out>, StreamError> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let outputs = (self.func)(value)?;
    Ok(outputs.into_iter().map(|output| Box::new(output) as DynValue).collect())
  }
}
