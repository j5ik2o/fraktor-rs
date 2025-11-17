//! Scheduler utilities specialised for the standard toolbox runtime.

/// Tick driver integrations for standard runtimes.
#[cfg(feature = "tokio-executor")]
pub mod tick;
