//! Tick driver configuration and abstractions.

mod auto_driver_metadata;
mod bootstrap;
mod bootstrap_provision_result;
mod scheduler_tick_executor;
mod scheduler_tick_handle_owned;
#[cfg(any(test, feature = "test-support"))]
mod test_tick_driver;
mod tick_driver_bundle;
mod tick_driver_error;
mod tick_driver_id;
mod tick_driver_kind;
mod tick_driver_metadata;
mod tick_driver_provision;
mod tick_driver_provisioning_context;
mod tick_driver_stopper;
mod tick_driver_trait;
mod tick_executor_signal;
mod tick_feed;
mod tick_metrics;
mod tick_metrics_probe;

#[cfg(test)]
mod tests;

pub use auto_driver_metadata::{AutoDriverMetadata, AutoProfileKind};
#[cfg(any(test, feature = "test-support"))]
pub use bootstrap::TickDriverBootstrap;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use bootstrap::TickDriverBootstrap;
pub use bootstrap_provision_result::BootstrapProvisionResult;
pub use scheduler_tick_executor::SchedulerTickExecutor;
pub(crate) use scheduler_tick_handle_owned::SchedulerTickHandleOwned;
#[cfg(any(test, feature = "test-support"))]
pub use test_tick_driver::TestTickDriver;
pub use tick_driver_bundle::TickDriverBundle;
pub use tick_driver_error::TickDriverError;
pub use tick_driver_id::TickDriverId;
pub use tick_driver_kind::TickDriverKind;
pub use tick_driver_metadata::TickDriverMetadata;
pub use tick_driver_provision::TickDriverProvision;
pub use tick_driver_provisioning_context::TickDriverProvisioningContext;
pub use tick_driver_stopper::TickDriverStopper;
pub use tick_driver_trait::{TickDriver, next_tick_driver_id};
pub use tick_executor_signal::TickExecutorSignal;
pub use tick_feed::{TickFeed, TickFeedHandle};
pub use tick_metrics::SchedulerTickMetrics;
pub use tick_metrics_probe::SchedulerTickMetricsProbe;
