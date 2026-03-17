//! Configuration for the consumer controller.

#[cfg(test)]
mod tests;

/// Default flow-control window size.
const DEFAULT_FLOW_CONTROL_WINDOW: u32 = 50;

/// Settings for [`ConsumerController`](super::ConsumerController).
#[derive(Debug, Clone)]
pub struct ConsumerControllerSettings {
  flow_control_window: u32,
  only_flow_control:   bool,
}

impl ConsumerControllerSettings {
  /// Creates default settings for in-memory reliable delivery.
  #[must_use]
  pub const fn new() -> Self {
    Self { flow_control_window: DEFAULT_FLOW_CONTROL_WINDOW, only_flow_control: false }
  }

  /// Returns the flow-control window size.
  ///
  /// This determines how many unconfirmed messages the consumer side will
  /// request from the producer side at a time.
  #[must_use]
  pub const fn flow_control_window(&self) -> u32 {
    self.flow_control_window
  }

  /// Returns a new settings with the given flow-control window.
  #[must_use]
  pub const fn with_flow_control_window(self, window: u32) -> Self {
    let clamped = if window == 0 { 1 } else { window };
    Self { flow_control_window: clamped, ..self }
  }

  /// Returns whether only flow-control is used (no resend of lost messages).
  #[must_use]
  pub const fn only_flow_control(&self) -> bool {
    self.only_flow_control
  }

  /// Returns a new settings with `only_flow_control` set.
  #[must_use]
  pub const fn with_only_flow_control(self, value: bool) -> Self {
    Self { only_flow_control: value, ..self }
  }
}

impl Default for ConsumerControllerSettings {
  fn default() -> Self {
    Self::new()
  }
}
