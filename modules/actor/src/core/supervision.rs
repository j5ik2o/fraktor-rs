//! Supervision package.
//!
//! This module contains error handling and restart strategies.

mod base;
mod restart_statistics;
mod supervisor_directive;
mod supervisor_strategy_kind;

pub use base::SupervisorStrategy;
pub use restart_statistics::RestartStatistics;
pub use supervisor_directive::SupervisorDirective;
pub use supervisor_strategy_kind::SupervisorStrategyKind;
