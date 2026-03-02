//! Logging bindings for standard runtimes.

mod tracing_logger_subscriber;

pub use tracing_logger_subscriber::TracingLoggerSubscriber;
/// Standard-runtime alias for the core logger subscriber.
pub type StdLoggerSubscriber = crate::core::event::logging::LoggerSubscriber;
