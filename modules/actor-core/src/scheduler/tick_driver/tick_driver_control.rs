//! Driver control hook invoked for shutdown.

/// Control hook invoked when the driver needs to stop.
pub trait TickDriverControl: Send + Sync + 'static {
  /// Stops the driver and cleans up associated resources.
  fn shutdown(&self);
}
