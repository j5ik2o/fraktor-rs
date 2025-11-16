//! Tick driver configuration and abstractions.

mod auto_driver_config;
mod auto_driver_metadata;
mod auto_profile_kind;
mod bootstrap;
mod fallback_policy;
mod hardware_driver;
mod hardware_kind;
#[cfg(any(test, feature = "test-support"))]
mod manual_test_driver;
mod scheduler_tick_executor;
mod scheduler_tick_handle_owned;
mod tick_driver_auto_locator;
mod tick_driver_config;
mod tick_driver_control;
mod tick_driver_error;
mod tick_driver_factory;
mod tick_driver_handle;
mod tick_driver_id;
mod tick_driver_kind;
mod tick_driver_matrix;
mod tick_driver_metadata;
mod tick_driver_runtime;
mod tick_driver_trait;
mod tick_executor_signal;
mod tick_executor_signal_future;
mod tick_feed;
mod tick_metrics;
mod tick_metrics_mode;
mod tick_metrics_probe;
mod tick_pulse_handler;
mod tick_pulse_source;

#[cfg(test)]
mod tests;

pub use auto_driver_config::AutoDriverConfig;
pub use auto_driver_metadata::AutoDriverMetadata;
pub use auto_profile_kind::AutoProfileKind;
pub use bootstrap::TickDriverBootstrap;
pub use fallback_policy::FallbackPolicy;
pub use hardware_kind::HardwareKind;
#[cfg(any(test, feature = "test-support"))]
pub use manual_test_driver::ManualTestDriver;
pub use scheduler_tick_executor::SchedulerTickExecutor;
pub use scheduler_tick_handle_owned::SchedulerTickHandleOwned;
pub use tick_driver_auto_locator::{TickDriverAutoLocator, TickDriverAutoLocatorRef};
pub use tick_driver_config::TickDriverConfig;
pub use tick_driver_control::TickDriverControl;
pub use tick_driver_error::TickDriverError;
pub use tick_driver_factory::{TickDriverFactory, TickDriverFactoryRef};
pub use tick_driver_handle::TickDriverHandle;
pub use tick_driver_id::TickDriverId;
pub use tick_driver_kind::TickDriverKind;
pub use tick_driver_matrix::{TICK_DRIVER_MATRIX, TickDriverGuideEntry};
pub use tick_driver_metadata::TickDriverMetadata;
pub use tick_driver_runtime::TickDriverRuntime;
pub use tick_driver_trait::{TickDriver, next_tick_driver_id};
pub use tick_executor_signal::TickExecutorSignal;
pub use tick_feed::{TickFeed, TickFeedHandle};
pub use tick_metrics::SchedulerTickMetrics;
pub use tick_metrics_mode::TickMetricsMode;
pub use tick_metrics_probe::SchedulerTickMetricsProbe;
pub use tick_pulse_handler::TickPulseHandler;
pub use tick_pulse_source::TickPulseSource;
