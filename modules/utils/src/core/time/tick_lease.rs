//! Lease that drains pending ticks.

use core::marker::PhantomData;

use super::{tick_event::TickEvent, tick_state::TickState};
use crate::core::sync::ArcShared;

/// Lease borrowed from a tick handle to consume accumulated ticks.
pub struct TickLease<'a> {
  state:  ArcShared<TickState>,
  _scope: PhantomData<&'a TickState>,
}

impl TickLease<'_> {
  pub(crate) const fn new(state: ArcShared<TickState>) -> Self {
    Self { state, _scope: PhantomData }
  }

  /// Attempts to pull a pending tick event.
  #[must_use]
  pub fn try_pull(&self) -> Option<TickEvent> {
    let ticks = self.state.take();
    if ticks == 0 { None } else { Some(TickEvent::new(ticks)) }
  }
}
