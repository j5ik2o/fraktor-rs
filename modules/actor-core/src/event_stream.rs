//! Event stream placeholder.

use core::marker::PhantomData;

use crate::{NoStdToolbox, RuntimeToolbox};

/// Broadcasts runtime events placeholder.
pub struct EventStreamGeneric<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamGeneric<TB> {
  fn default() -> Self {
    Self { _marker: PhantomData }
  }
}

/// 既定ツールボックス向けの型エイリアス。
pub type EventStream = EventStreamGeneric<NoStdToolbox>;
