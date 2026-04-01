//! Supervision package.
//!
//! This module contains error handling and restart strategies.

mod backoff_supervisor_strategy;
mod base;
mod restart_statistics;
mod supervisor_directive;
mod supervisor_strategy_config;
mod supervisor_strategy_kind;

pub use backoff_supervisor_strategy::BackoffSupervisorStrategy;
pub use base::SupervisorStrategy;
pub use restart_statistics::RestartStatistics;
pub use supervisor_directive::SupervisorDirective;
pub use supervisor_strategy_config::SupervisorStrategyConfig;
pub use supervisor_strategy_kind::SupervisorStrategyKind;

mod backoff_on_failure_options;
mod backoff_on_stop_options;
mod backoff_supervisor;
mod backoff_supervisor_command;
mod backoff_supervisor_response;

pub use backoff_on_failure_options::BackoffOnFailureOptions;
pub use backoff_on_stop_options::BackoffOnStopOptions;
pub use backoff_supervisor::BackoffSupervisor;
pub use backoff_supervisor_command::BackoffSupervisorCommand;
pub use backoff_supervisor_response::BackoffSupervisorResponse;
