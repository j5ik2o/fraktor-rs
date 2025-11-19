//! Dispatcher module providing scheduling primitives.
//!
//! This module contains the core dispatching infrastructure for actor task execution.
//! Following the Pekko/Akka architecture, `dispatcher` is an independent top-level package
//! rather than a sub-module of `system`.
//!
//! # Architecture
//!
//! - **Pekko**: `org.apache.pekko.dispatch` (independent package)
//! - **Akka**: `akka.dispatch` (independent package)
//! - **fraktor-rs**: `fraktor_core::dispatcher` (independent module)
//!
//! The dispatcher manages message processing and task scheduling for actors, working in
//! conjunction with the `system` module but maintaining separate responsibilities:
//! - `system`: System lifecycle and management
//! - `dispatcher`: Task execution and scheduling infrastructure

mod base;
mod dispatch_error;
mod dispatch_executor;
mod dispatch_shared;
mod dispatcher_core;
mod dispatcher_dump_event;
mod dispatcher_sender;
mod dispatcher_state;
mod inline_executor;
mod inline_schedule_adapter;
mod schedule_adapter;
mod schedule_waker;
mod tick_executor;

pub use base::{Dispatcher, DispatcherGeneric};
pub use dispatch_error::DispatchError;
pub use dispatch_executor::DispatchExecutor;
pub use dispatch_shared::{DispatchShared, DispatchSharedGeneric};
pub use dispatcher_config::{DispatcherConfig, DispatcherConfigGeneric};
pub use dispatcher_dump_event::DispatcherDumpEvent;
pub use dispatcher_sender::{DispatcherSender, DispatcherSenderGeneric};
pub use inline_executor::{InlineExecutor, InlineExecutorGeneric};
pub use inline_schedule_adapter::InlineScheduleAdapter;
pub use schedule_adapter::ScheduleAdapter;
pub use tick_executor::{TickExecutor, TickExecutorGeneric};

/// Dispatcher configuration module.
mod dispatcher_config;
#[cfg(test)]
mod tests;
