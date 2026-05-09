//! Classic actor logging facade.

#[cfg(test)]
mod tests;

use crate::{actor::ActorContext, event::logging::LoggingAdapter};

/// Provides a context-bound classic logging adapter under the name `log`.
#[derive(Clone)]
pub struct ActorLogging {
  adapter: LoggingAdapter,
}

impl ActorLogging {
  /// Creates a new classic logging facade for the provided actor context.
  #[must_use]
  pub fn new(context: &ActorContext<'_>) -> Self {
    Self { adapter: LoggingAdapter::from_context(context) }
  }

  /// Returns the classic logging adapter.
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
