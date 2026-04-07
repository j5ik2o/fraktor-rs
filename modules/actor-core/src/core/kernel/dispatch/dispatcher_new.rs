//! New dispatcher module aligned with Apache Pekko's `MessageDispatcher` model.
//!
//! This module is being introduced in parallel with the legacy `dispatcher` module
//! during the dispatcher redesign. Files inside `dispatcher_new/` MUST NOT depend on
//! anything from the legacy `dispatcher/` tree (see openspec change
//! `dispatcher-pekko-1n-redesign`).
//!
//! Once all callers have migrated, the legacy `dispatcher/` tree is removed in a
//! single drop and `dispatcher_new/` is renamed back to `dispatcher/`.

mod balancing_dispatcher;
mod balancing_dispatcher_configurator;
mod default_dispatcher;
mod default_dispatcher_configurator;
mod dispatcher_core;
mod dispatcher_settings;
mod dispatcher_waker;
mod dispatchers;
mod dispatchers_error;
mod execute_error;
mod executor;
mod executor_factory;
mod executor_shared;
mod inline_executor;
mod message_dispatcher;
mod message_dispatcher_configurator;
mod message_dispatcher_shared;
mod new_dispatcher_sender;
mod pinned_dispatcher;
mod pinned_dispatcher_configurator;
mod shared_message_queue;
mod shutdown_schedule;

pub use balancing_dispatcher::BalancingDispatcher;
pub use balancing_dispatcher_configurator::BalancingDispatcherConfigurator;
pub use default_dispatcher::DefaultDispatcher;
pub use default_dispatcher_configurator::DefaultDispatcherConfigurator;
pub use dispatcher_core::DispatcherCore;
pub use dispatcher_settings::DispatcherSettings;
pub use dispatcher_waker::dispatcher_waker;
pub use dispatchers::{DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, Dispatchers};
pub use dispatchers_error::DispatchersError;
pub use execute_error::ExecuteError;
pub use executor::Executor;
pub use executor_factory::ExecutorFactory;
pub use executor_shared::ExecutorShared;
pub use inline_executor::InlineExecutor;
pub use message_dispatcher::MessageDispatcher;
pub use message_dispatcher_configurator::MessageDispatcherConfigurator;
pub use message_dispatcher_shared::MessageDispatcherShared;
pub use new_dispatcher_sender::NewDispatcherSender;
pub use pinned_dispatcher::PinnedDispatcher;
pub use pinned_dispatcher_configurator::PinnedDispatcherConfigurator;
pub use shared_message_queue::SharedMessageQueue;
pub use shutdown_schedule::ShutdownSchedule;
