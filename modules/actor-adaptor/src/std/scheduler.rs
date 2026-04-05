//! Scheduler utilities specialised for the standard toolbox runtime.

/// Tick driver integrations for standard runtimes.
#[cfg(feature = "tokio-executor")]
mod tick;

#[cfg(feature = "tokio-executor")]
pub use tick::{default_tick_driver_config, tick_driver_config_with_resolution};
