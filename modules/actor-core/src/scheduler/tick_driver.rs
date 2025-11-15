//! Tick driver configuration and abstractions.

mod auto_driver_config;
mod fallback_policy;
#[cfg(any(test, feature = "test-support"))]
mod manual_test_driver;
mod tick_driver_auto_locator;
mod tick_driver_config;
mod tick_driver_error;
mod tick_driver_factory;
mod tick_driver_kind;
mod tick_driver_metadata;
mod tick_metrics_mode;
mod tick_pulse_source;

pub use auto_driver_config::AutoDriverConfig;
pub use fallback_policy::FallbackPolicy;
#[cfg(any(test, feature = "test-support"))]
pub use manual_test_driver::ManualTestDriver;
pub use tick_driver_auto_locator::{TickDriverAutoLocator, TickDriverAutoLocatorRef};
pub use tick_driver_config::TickDriverConfig;
pub use tick_driver_error::TickDriverError;
pub use tick_driver_factory::{TickDriverFactory, TickDriverFactoryRef};
pub use tick_driver_kind::{HardwareKind, TickDriverKind};
pub use tick_driver_metadata::{AutoDriverMetadata, AutoProfileKind, TickDriverId, TickDriverMetadata};
pub use tick_metrics_mode::TickMetricsMode;
pub use tick_pulse_source::{TickPulseHandler, TickPulseSource};
