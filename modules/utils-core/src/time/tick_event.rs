//! Tick events produced by the toolbox driver.

/// Reports how many ticks accumulated since the previous lease poll.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TickEvent {
  ticks: u32,
}

impl TickEvent {
  /// Creates a new event.
  #[must_use]
  pub const fn new(ticks: u32) -> Self {
    Self { ticks }
  }

  /// Number of pending ticks represented by the event.
  #[must_use]
  pub const fn ticks(&self) -> u32 {
    self.ticks
  }
}
