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
//! - **cellactor-rs**: `cellactor_core::dispatcher` (independent module)
//!
//! The dispatcher manages message processing and task scheduling for actors, working in
//! conjunction with the `system` module but maintaining separate responsibilities:
//! - `system`: System lifecycle and management
//! - `dispatcher`: Task execution and scheduling infrastructure

mod dispatch_executor;
mod dispatch_handle;
mod dispatcher_core;
mod dispatcher_sender;
mod dispatcher_state;
mod base;
mod inline_executor;
mod schedule_waker;

pub use dispatch_executor::DispatchExecutor;
pub use dispatch_handle::DispatchHandle;
pub use dispatcher_sender::DispatcherSender;
pub use base::Dispatcher;
pub use inline_executor::InlineExecutor;

#[cfg(test)]
mod tests;
