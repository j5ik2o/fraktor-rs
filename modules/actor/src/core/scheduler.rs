//! Scheduler subsystem providing Pekko-compatible APIs.

mod batch_mode;
/// Cancellable entry, registry, and state types.
pub mod cancellable;
mod command;
mod config;
mod delay_provider;
/// Deterministic event logging and replay types.
pub mod deterministic;
/// Scheduler diagnostics subsystem types.
pub mod diagnostics;
mod dispatcher_sender_shared;
mod dump;
mod dump_job;
mod error;
mod execution_batch;
mod handle;
mod metrics;
mod mode;
/// Scheduler policy types for periodic job execution.
pub mod policy;
mod runnable;
mod runner_mode;
mod scheduler_context;
mod scheduler_core;
mod scheduler_runner;
mod scheduler_runner_owned;
mod scheduler_shared;
/// Task run entry, handle, and related types.
pub mod task_run;
/// Tick driver subsystem.
pub mod tick_driver;
mod warning;

#[cfg(test)]
mod tests;

pub use batch_mode::BatchMode;
pub use command::SchedulerCommand;
pub use config::SchedulerConfig;
pub use delay_provider::SchedulerBackedDelayProvider;
pub use dispatcher_sender_shared::DispatcherSenderShared;
pub use dump::SchedulerDump;
pub use dump_job::SchedulerDumpJob;
pub use error::SchedulerError;
pub use execution_batch::ExecutionBatch;
pub use handle::SchedulerHandle;
pub use metrics::SchedulerMetrics;
pub use mode::SchedulerMode;
pub use runnable::SchedulerRunnable;
pub use runner_mode::RunnerMode;
pub use scheduler_context::SchedulerContext;
pub use scheduler_core::Scheduler;
pub use scheduler_runner::SchedulerRunner;
pub use scheduler_runner_owned::SchedulerRunnerOwned;
pub use scheduler_shared::{SchedulerShared, SchedulerSharedGeneric};
pub use warning::SchedulerWarning;
