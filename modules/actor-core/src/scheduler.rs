//! Scheduler subsystem providing Pekko-compatible APIs.

pub mod api;
pub mod cancellable;
pub mod cancellable_registry;
pub mod command;
pub mod config;
pub mod dispatcher_sender_shared;
pub mod error;
pub mod execution_batch;
mod diagnostics;
mod fixed_delay_context;
pub mod fixed_delay_policy;
mod fixed_rate_context;
pub mod fixed_rate_policy;
pub mod handle;
pub mod metrics;
pub mod mode;
pub mod delay_provider;
mod periodic_batch_decision;
pub mod policy_registry;
pub mod scheduler_context;
pub mod task_run;
pub mod runnable;
pub mod runner;
pub mod scheduler_core;
pub mod warning;

#[cfg(test)]
mod tests;

pub use command::SchedulerCommand;
pub use config::SchedulerConfig;
pub use dispatcher_sender_shared::DispatcherSenderShared;
pub use error::SchedulerError;
pub use delay_provider::SchedulerBackedDelayProvider;
pub use diagnostics::{DeterministicEvent, SchedulerDiagnostics};
pub use execution_batch::ExecutionBatch;
pub use fixed_delay_policy::FixedDelayPolicy;
pub use fixed_rate_policy::FixedRatePolicy;
pub use handle::SchedulerHandle;
pub use metrics::SchedulerMetrics;
pub use mode::SchedulerMode;
pub use policy_registry::SchedulerPolicyRegistry;
pub use scheduler_context::SchedulerContext;
pub use task_run::{TaskRunError, TaskRunHandle, TaskRunOnClose, TaskRunPriority, TaskRunSummary};
pub use runnable::SchedulerRunnable;
pub use runner::{RunnerMode, SchedulerRunner};
pub use scheduler_core::Scheduler;
pub use warning::SchedulerWarning;
