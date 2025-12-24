//! Logging bindings for standard runtimes.

mod tracing_logger_subscriber;

use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;
pub use tracing_logger_subscriber::TracingLoggerSubscriber;

/// Type alias for `LoggerSubscriberGeneric` with `StdToolbox`.
pub type StdLoggerSubscriber = crate::core::event::logging::LoggerSubscriberGeneric<StdToolbox>;
