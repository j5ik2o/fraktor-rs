/// Runner operating mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerMode {
  /// Manual driver using deterministic tick injection.
  Manual,
  /// Placeholder for async host drivers (tokio, std timers).
  AsyncHost,
  /// Placeholder for hardware-backed drivers (embassy/SysTick).
  Hardware,
}
