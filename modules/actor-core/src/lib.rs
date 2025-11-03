#![deny(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_types, clippy::redundant_clone))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![cfg_attr(not(test), deny(clippy::redundant_clone))]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enum)]
#![deny(clippy::no_effect)]
#![deny(dropping_copy_types)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![no_std]

//! Core utility collection.
//!
//! Provides fundamental data structures such as mailboxes, synchronization primitives,
//! and deadline-based processing intended for cross-runtime sharing, with `no_std` support.
//! By interacting with `actor-core` through this crate, we maintain unidirectional dependencies,
//! and each runtime only needs to satisfy the abstractions defined here with their own
//! implementations.

extern crate alloc;

pub use cellactor_utils_core_rs::sync::{NoStdMutex, NoStdToolbox, RuntimeToolbox, ToolboxMutex};

mod actor;
mod actor_cell;
mod actor_context;
mod actor_error;
mod actor_error_reason;
mod actor_future;
mod actor_future_listener;
mod actor_ref;
mod any_message;
mod any_message_view;
mod ask_response;
mod child_ref;
mod deadletter;
mod deadletter_entry;
mod deadletter_reason;
mod dispatcher;
mod event_stream;
mod event_stream_event;
mod event_stream_subscriber;
mod event_stream_subscriber_entry;
mod event_stream_subscription;
mod lifecycle_event;
mod lifecycle_stage;
mod log_event;
mod log_level;
mod logger_subscriber;
mod logger_writer;
mod mailbox;
mod mailbox_capacity;
mod mailbox_metrics_event;
mod mailbox_overflow_strategy;
mod mailbox_policy;
mod message_invoker;
mod name_registry;
mod name_registry_error;
mod pid;
mod props_actor_factory;
mod props_dispatcher_config;
mod props_mailbox_config;
mod props_struct;
mod props_supervisor_options;
mod receive_state;
mod restart_statistics;
mod send_error;
mod spawn_error;
mod supervisor_strategy;
mod system;
mod system_message;
mod system_state;

pub use actor::Actor;
pub use actor_cell::ActorCell;
pub use actor_context::ActorContext;
pub use actor_error::ActorError;
pub use actor_error_reason::ActorErrorReason;
pub use actor_future::ActorFuture;
pub use actor_future_listener::ActorFutureListener;
pub use actor_ref::{ActorRef, ActorRefSender, AskReplySender, NullSender};
pub use any_message::AnyMessage;
pub use any_message_view::AnyMessageView;
pub use ask_response::AskResponse;
pub use child_ref::ChildRef;
pub use deadletter::{Deadletter, DeadletterGeneric};
pub use deadletter_entry::DeadletterEntry;
pub use deadletter_reason::DeadletterReason;
pub use dispatcher::{DispatchExecutor, DispatchHandle, Dispatcher};
pub use event_stream::{EventStream, EventStreamGeneric};
pub use event_stream_event::EventStreamEvent;
pub use event_stream_subscriber::EventStreamSubscriber;
pub use event_stream_subscription::{EventStreamSubscription, EventStreamSubscriptionGeneric};
pub use lifecycle_event::LifecycleEvent;
pub use lifecycle_stage::LifecycleStage;
pub use log_event::LogEvent;
pub use log_level::LogLevel;
pub use logger_subscriber::LoggerSubscriber;
pub use logger_writer::LoggerWriter;
pub use mailbox::{
  EnqueueOutcome, Mailbox, MailboxInstrumentation, MailboxMessage, MailboxOfferFuture, MailboxPollFuture,
};
pub use mailbox_capacity::MailboxCapacity;
pub use mailbox_metrics_event::MailboxMetricsEvent;
pub use mailbox_overflow_strategy::MailboxOverflowStrategy;
pub use mailbox_policy::MailboxPolicy;
pub use message_invoker::{MessageInvoker, MessageInvokerMiddleware, MessageInvokerPipeline};
pub use name_registry::NameRegistry;
pub use name_registry_error::NameRegistryError;
pub use pid::Pid;
pub use props_actor_factory::ActorFactory;
pub use props_dispatcher_config::DispatcherConfig;
pub use props_mailbox_config::MailboxConfig;
pub use props_struct::Props;
pub use props_supervisor_options::SupervisorOptions;
pub use receive_state::ReceiveState;
pub use restart_statistics::RestartStatistics;
pub use send_error::SendError;
pub use spawn_error::SpawnError;
pub use supervisor_strategy::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind};
pub use system::ActorSystemGeneric;
pub use system_message::SystemMessage;
pub use system_state::SystemState;

/// Type alias for ActorSystem using the default toolbox.
pub type ActorSystem = ActorSystemGeneric<NoStdToolbox>;
