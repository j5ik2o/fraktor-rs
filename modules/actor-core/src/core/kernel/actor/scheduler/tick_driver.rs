//! Tick driver configuration and abstractions.

mod auto_driver_metadata;
mod bootstrap;
mod hardware_driver;
#[cfg(any(test, feature = "test-support"))]
mod manual_test_driver;
#[cfg(any(test, feature = "test-support"))]
mod manual_tick_controller;
mod scheduler_tick_executor;
mod scheduler_tick_handle_owned;
mod tick_driver_bundle;
mod tick_driver_config;
mod tick_driver_control;
mod tick_driver_control_shared;
mod tick_driver_control_shared_factory;
mod tick_driver_error;
mod tick_driver_handle;
mod tick_driver_id;
mod tick_driver_kind;
mod tick_driver_metadata;
mod tick_driver_provisioning_context;
mod tick_driver_trait;
mod tick_executor_pump;
mod tick_executor_signal;
mod tick_feed;
mod tick_metrics;
mod tick_metrics_probe;
mod tick_pulse_handler;
mod tick_pulse_source;

#[cfg(test)]
mod tests;

pub use auto_driver_metadata::{AutoDriverMetadata, AutoProfileKind};
#[cfg(any(test, feature = "test-support"))]
pub use bootstrap::TickDriverBootstrap;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use bootstrap::TickDriverBootstrap;
pub use hardware_driver::HardwareTickDriver;
#[cfg(any(test, feature = "test-support"))]
pub use manual_test_driver::ManualTestDriver;
#[cfg(any(test, feature = "test-support"))]
pub use manual_tick_controller::ManualTickController;
pub use scheduler_tick_executor::SchedulerTickExecutor;
pub(crate) use scheduler_tick_handle_owned::SchedulerTickHandleOwned;
pub use tick_driver_bundle::TickDriverBundle;
pub use tick_driver_config::TickDriverConfig;
pub use tick_driver_control::TickDriverControl;
pub use tick_driver_control_shared::TickDriverControlShared;
pub use tick_driver_control_shared_factory::TickDriverControlSharedFactory;
pub use tick_driver_error::TickDriverError;
pub use tick_driver_handle::TickDriverHandle;
pub use tick_driver_id::TickDriverId;
pub use tick_driver_kind::{HardwareKind, TickDriverKind};
pub use tick_driver_metadata::TickDriverMetadata;
pub use tick_driver_provisioning_context::TickDriverProvisioningContext;
pub use tick_driver_trait::{TickDriver, next_tick_driver_id};
pub use tick_executor_pump::TickExecutorPump;
pub use tick_executor_signal::TickExecutorSignal;
pub use tick_feed::{TickFeed, TickFeedHandle};
pub use tick_metrics::SchedulerTickMetrics;
pub use tick_metrics_probe::SchedulerTickMetricsProbe;
pub use tick_pulse_handler::TickPulseHandler;
pub use tick_pulse_source::TickPulseSource;
