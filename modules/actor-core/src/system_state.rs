//! Actor system state placeholder.

use core::marker::PhantomData;

use crate::{NoStdToolbox, RuntimeToolbox};

/// Captures global actor system state.
pub struct SystemState<TB: RuntimeToolbox = NoStdToolbox> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox> Default for SystemState<TB> {
  fn default() -> Self {
    Self { _marker: PhantomData }
  }
}
