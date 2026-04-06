//! Dispatcher module providing scheduling primitives.
//!
//! This module contains the core dispatching infrastructure for actor task execution.
//! Following the Pekko/Akka architecture, `dispatcher` stays independent from `system`,
//! and in fraktor-rs it is grouped under `dispatch` alongside mailbox types.
//!
//! # Architecture
//!
//! - **Pekko**: `org.apache.pekko.dispatch` (independent package)
//! - **Akka**: `akka.dispatch` (independent package)
//! - **fraktor-rs**: `fraktor_core::dispatch::dispatcher`
//!
//! The dispatcher manages message processing and task scheduling for actors, working in
//! conjunction with the `system` module but maintaining separate responsibilities:
//! - `system`: System lifecycle and management
//! - `dispatcher`: Task execution and scheduling infrastructure

mod dispatch_error;
mod dispatch_executor;
mod dispatch_executor_runner;
mod dispatch_shared;
mod dispatcher_builder;
mod dispatcher_core;
mod dispatcher_dump_event;
mod dispatcher_provider;
mod dispatcher_provision_request;
mod dispatcher_registry_entry;
mod dispatcher_registry_error;
mod dispatcher_sender;
mod dispatcher_settings;
mod dispatcher_shared;
mod dispatcher_state;
mod dispatchers;
mod inline_dispatcher_provider;
mod inline_executor;
mod inline_schedule_adapter;
mod schedule_adapter;
mod schedule_adapter_shared;
mod schedule_waker;
mod tick_executor;

#[doc(hidden)]
pub use configured_dispatcher_builder::ConfiguredDispatcherBuilder;
#[doc(hidden)]
pub use dispatch_error::DispatchError;
#[doc(hidden)]
pub use dispatch_executor::DispatchExecutor;
#[doc(hidden)]
pub use dispatch_executor_runner::DispatchExecutorRunner;
#[doc(hidden)]
pub use dispatch_shared::DispatchShared;
pub use dispatcher_builder::DispatcherBuilder;
pub use dispatcher_dump_event::DispatcherDumpEvent;
pub use dispatcher_provider::DispatcherProvider;
pub use dispatcher_provision_request::DispatcherProvisionRequest;
pub use dispatcher_registry_entry::DispatcherRegistryEntry;
pub use dispatcher_registry_error::DispatcherRegistryError;
pub(crate) use dispatcher_sender::DispatcherSender;
pub use dispatcher_settings::DispatcherSettings;
#[doc(hidden)]
pub use dispatcher_shared::DispatcherShared;
pub use dispatchers::{DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, Dispatchers};
pub(crate) use inline_dispatcher_provider::InlineDispatcherProvider;
pub(crate) use inline_executor::InlineExecutor;
#[doc(hidden)]
pub use inline_schedule_adapter::InlineScheduleAdapter;
pub use schedule_adapter::ScheduleAdapter;
#[doc(hidden)]
pub use schedule_adapter_shared::ScheduleAdapterShared;
#[doc(hidden)]
pub use tick_executor::TickExecutor;

/// Dispatcher configuration module.
mod configured_dispatcher_builder;
#[cfg(test)]
mod tests;
