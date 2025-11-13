use core::time::Duration;

mod delay_future;
mod delay_trigger;
mod manual_delay_provider;

pub use delay_future::DelayFuture;
pub(crate) use delay_future::DelayState;
pub use delay_trigger::DelayTrigger;
pub use manual_delay_provider::ManualDelayProvider;

/// Provider capable of creating delay futures backed by the current runtime.
pub trait DelayProvider: Send + Sync + 'static {
  /// Returns a future that completes after the specified duration.
  fn delay(&self, duration: Duration) -> DelayFuture;
}
