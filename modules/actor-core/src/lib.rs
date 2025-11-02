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
mod ask_response;
mod any_message;
mod any_message_view;
mod child_ref;
mod deadletter;
mod dispatcher;
mod event_stream;
mod logger_subscriber;
mod mailbox;
mod mailbox_capacity;
mod mailbox_overflow_strategy;
mod mailbox_policy;
mod message_invoker;
mod name_registry;
mod name_registry_error;
mod pid;
mod props;
mod receive_state;
mod restart_statistics;
mod send_error;
mod supervisor_strategy;
mod system;
mod system_state;

pub use actor::Actor;
pub use actor_cell::ActorCell;
pub use actor_context::ActorContext;
pub use actor_error::ActorError;
pub use actor_error_reason::ActorErrorReason;
pub use actor_future::ActorFuture;
pub use actor_future_listener::ActorFutureListener;
pub use actor_ref::ActorRef;
pub use ask_response::AskResponse;
pub use any_message::AnyMessage;
pub use any_message_view::AnyMessageView;
pub use child_ref::ChildRef;
pub use deadletter::Deadletter;
pub use dispatcher::Dispatcher;
pub use event_stream::EventStream;
pub use logger_subscriber::LoggerSubscriber;
pub use mailbox::Mailbox;
pub use mailbox_capacity::MailboxCapacity;
pub use mailbox_overflow_strategy::MailboxOverflowStrategy;
pub use mailbox_policy::MailboxPolicy;
pub use message_invoker::MessageInvoker;
pub use name_registry::NameRegistry;
pub use name_registry_error::NameRegistryError;
pub use pid::Pid;
pub use props::Props;
pub use receive_state::ReceiveState;
pub use restart_statistics::RestartStatistics;
pub use send_error::SendError;
pub use supervisor_strategy::SupervisorStrategy;
pub use system::ActorSystem;
pub use system_state::SystemState;
