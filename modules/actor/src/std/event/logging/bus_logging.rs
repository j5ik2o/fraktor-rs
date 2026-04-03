//! Bus-oriented classic logging facade.

#[cfg(test)]
mod tests;

use alloc::string::String;

use super::logging_adapter::LoggingAdapter;
use crate::core::kernel::{actor::Pid, system::ActorSystem};

/// Classic logging facade for non-actor event-bus style publishers.
#[derive(Clone)]
pub struct BusLogging {
  adapter: LoggingAdapter,
}

impl BusLogging {
  /// Creates a new bus logging facade.
  #[must_use]
  pub const fn new(system: ActorSystem, origin: Option<Pid>, logger_name: Option<String>) -> Self {
    Self { adapter: LoggingAdapter::new(system, origin, logger_name) }
  }

  /// Returns the underlying classic logging adapter.
  #[must_use]
  pub const fn log(&mut self) -> &mut LoggingAdapter {
    &mut self.adapter
  }

  /// Consumes the facade and returns the underlying adapter.
  #[must_use]
  pub fn into_log(self) -> LoggingAdapter {
    self.adapter
  }
}
