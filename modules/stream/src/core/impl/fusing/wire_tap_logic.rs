use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

/// Wire-tap logic that clones each element to a secondary (tap) output.
///
/// Unlike `BroadcastLogic`, this stage semantically represents a
/// fire-and-forget tap: the main output always receives the element,
/// and the tap output receives a clone. In the current tick-based
/// execution model both outputs are processed synchronously, but the
/// dedicated stage kind preserves the Pekko contract distinction
/// (tap never back-pressures main) for future async island support.
pub(in crate::core) struct WireTapLogic<Out> {
  pub(in crate::core) _pd: PhantomData<fn(Out)>,
}

impl<Out> FlowLogic for WireTapLogic<Out>
where
  Out: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Out>(input)?;
    let tap_value = value.clone();
    Ok(alloc::vec![Box::new(value) as DynValue, Box::new(tap_value) as DynValue])
  }

  fn expected_fan_out(&self) -> Option<usize> {
    Some(2)
  }
}
