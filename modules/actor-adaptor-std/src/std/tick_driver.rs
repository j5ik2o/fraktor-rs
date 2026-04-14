//! Tick driver implementations for standard (std) environments.

mod std_tick_driver;
#[cfg(feature = "tokio-executor")]
mod tokio_tick_driver;

#[cfg(test)]
mod tests;

pub use std_tick_driver::StdTickDriver;
#[cfg(feature = "tokio-executor")]
pub use tokio_tick_driver::TokioTickDriver;
