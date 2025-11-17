//! Tick handle used by runtime toolboxes.

use core::marker::PhantomData;

use super::{tick_lease::TickLease, tick_state::TickState};
use crate::core::sync::ArcShared;

/// Handle that owns the shared tick state.
pub struct SchedulerTickHandle<'a> {
  state:  ArcShared<TickState>,
  _scope: PhantomData<&'a TickState>,
}

impl<'a> SchedulerTickHandle<'a> {
  /// Creates a handle scoped to the provided owner.
  #[must_use]
  pub fn scoped<T>(_owner: &'a T) -> Self {
    Self { state: ArcShared::new(TickState::new()), _scope: PhantomData }
  }

  /// Borrows a lease for draining ticks.
  #[must_use]
  pub fn lease(&self) -> TickLease<'_> {
    TickLease::new(self.state.clone())
  }

  /// Manually injects ticks for deterministic tests.
  pub fn inject_manual_ticks(&self, ticks: u32) {
    if ticks == 0 {
      return;
    }
    self.state.enqueue(ticks);
  }
}
