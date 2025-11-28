use core::time::Duration;

mod delay_future;
mod delay_state;
mod delay_trigger;
mod manual_delay_provider;

pub use delay_future::DelayFuture;
pub use delay_trigger::DelayTrigger;
pub use manual_delay_provider::ManualDelayProvider;

/// Provider capable of creating delay futures backed by the current runtime.
///
/// # Interior Mutability Removed
///
/// This trait uses `&mut self` to require external synchronization.
/// Implementations should not use interior mutability (e.g., `Mutex`, `RefCell`).
/// Callers must ensure exclusive access when invoking `delay()`.
pub trait DelayProvider: Send + 'static {
  /// Returns a future that completes after the specified duration.
  fn delay(&mut self, duration: Duration) -> DelayFuture;
}
