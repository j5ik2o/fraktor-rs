use portable_atomic::AtomicU8;

/// Enumeration representing the scheduler state of the dispatcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DispatcherState {
  /// Idle state.
  Idle    = 0,
  /// Running state.
  Running = 1,
}

impl DispatcherState {
  pub const fn as_u8(self) -> u8 {
    self as u8
  }

  pub fn store(self, atomic: &AtomicU8) {
    atomic.store(self.as_u8(), core::sync::atomic::Ordering::Release);
  }

  pub fn compare_exchange(
    expected: DispatcherState,
    desired: DispatcherState,
    atomic: &AtomicU8,
  ) -> Result<DispatcherState, DispatcherState> {
    let result = atomic.compare_exchange(
      expected.as_u8(),
      desired.as_u8(),
      core::sync::atomic::Ordering::AcqRel,
      core::sync::atomic::Ordering::Acquire,
    );
    result.map(DispatcherState::from_u8).map_err(DispatcherState::from_u8)
  }

  pub const fn from_u8(value: u8) -> DispatcherState {
    match value {
      | 0 => DispatcherState::Idle,
      | 1 => DispatcherState::Running,
      | _ => panic!("invalid dispatcher state value"),
    }
  }
}
