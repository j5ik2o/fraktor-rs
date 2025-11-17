//! Scheduler subsystem providing Pekko-compatible APIs.

mod batch_mode;
mod cancellable_entry;
mod cancellable_registry;
mod cancellable_state;
mod command;
mod config;
mod delay_provider;
mod deterministic_event;
mod deterministic_log;
mod deterministic_replay;
mod diagnostics_registry;
mod dispatcher_sender_shared;
mod dump;
mod dump_job;
mod error;
mod execution_batch;
mod fixed_delay_context;
mod fixed_delay_policy;
mod fixed_rate_context;
mod fixed_rate_policy;
mod handle;
mod metrics;
mod mode;
mod periodic_batch_decision;
mod policy_registry;
mod runnable;
mod runner_mode;
mod scheduler_context;
mod scheduler_core;
mod scheduler_diagnostics;
mod scheduler_diagnostics_event;
mod scheduler_diagnostics_subscription;
mod scheduler_runner;
mod scheduler_runner_owned;
mod task_run_entry;
mod task_run_error;
mod task_run_handle;
mod task_run_on_close;
mod task_run_priority;
mod task_run_summary;
mod tick_driver;
mod warning;

#[cfg(test)]
mod tests;

pub use batch_mode::BatchMode;
pub use cancellable_entry::CancellableEntry;
pub use cancellable_registry::CancellableRegistry;
pub use cancellable_state::CancellableState;
pub use command::SchedulerCommand;
pub use config::SchedulerConfig;
pub use delay_provider::SchedulerBackedDelayProvider;
pub use deterministic_event::DeterministicEvent;
pub use deterministic_replay::DeterministicReplay;
pub use dispatcher_sender_shared::DispatcherSenderShared;
pub use dump::SchedulerDump;
pub use dump_job::SchedulerDumpJob;
pub use error::SchedulerError;
pub use execution_batch::ExecutionBatch;
pub use fixed_delay_policy::FixedDelayPolicy;
pub use fixed_rate_policy::FixedRatePolicy;
pub use handle::SchedulerHandle;
pub use metrics::SchedulerMetrics;
pub use mode::SchedulerMode;
pub use policy_registry::SchedulerPolicyRegistry;
pub use runnable::SchedulerRunnable;
pub use runner_mode::RunnerMode;
pub use scheduler_context::SchedulerContext;
pub use scheduler_core::Scheduler;
pub use scheduler_diagnostics::SchedulerDiagnostics;
pub use scheduler_diagnostics_event::SchedulerDiagnosticsEvent;
pub use scheduler_diagnostics_subscription::SchedulerDiagnosticsSubscription;
pub use scheduler_runner::SchedulerRunner;
pub use scheduler_runner_owned::SchedulerRunnerOwned;
pub(crate) use task_run_entry::{TaskRunEntry, TaskRunQueue};
pub use task_run_error::TaskRunError;
pub use task_run_handle::TaskRunHandle;
pub use task_run_on_close::TaskRunOnClose;
pub use task_run_priority::TaskRunPriority;
pub use task_run_summary::TaskRunSummary;
#[cfg(any(test, feature = "test-support"))]
pub use tick_driver::ManualTestDriver;
pub use tick_driver::{
  AutoDriverMetadata, AutoProfileKind, HardwareKind, HardwareTickDriver, SchedulerTickExecutor,
  SchedulerTickHandleOwned, SchedulerTickMetrics, SchedulerTickMetricsProbe, TICK_DRIVER_MATRIX, TickDriver,
  TickDriverBootstrap, TickDriverConfig, TickDriverControl, TickDriverError, TickDriverFactory, TickDriverFactoryRef,
  TickDriverGuideEntry, TickDriverHandle, TickDriverId, TickDriverKind, TickDriverMetadata, TickDriverRuntime,
  TickExecutorSignal, TickFeed, TickFeedHandle, TickMetricsMode, TickPulseHandler, TickPulseSource,
  next_tick_driver_id,
};
pub use warning::SchedulerWarning;
