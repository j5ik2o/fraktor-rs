//! Supervision package.
//!
//! This module contains error handling and restart strategies.

mod base;
mod restart_statistics;
mod strategy;
mod supervisor_directive;
mod supervisor_strategy_kind;

pub use restart_statistics::RestartStatistics;
pub use strategy::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind};
