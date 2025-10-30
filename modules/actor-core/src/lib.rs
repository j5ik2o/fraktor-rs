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

//! Core utility collection.
//!
//! Provides fundamental data structures such as mailboxes, synchronization primitives,
//! and deadline-based processing intended for cross-runtime sharing, with `no_std` support.
//! By interacting with `actor-core` through this crate, we maintain unidirectional dependencies,
//! and each runtime only needs to satisfy the abstractions defined here with their own
//! implementations.

#![no_std]

extern crate alloc;

mod actor;
mod actor_cell;
mod actor_context;
mod actor_error;
mod actor_future;
mod actor_ref;
mod any_message;
mod ask_response;
mod child_ref;
mod dispatcher;
mod mailbox;
mod mailbox_policy;
mod message_invoker;
mod name_registry;
mod pid;
mod props;
mod receive_state;
mod restart_statistics;
mod send_error;
mod spawn_error;
mod supervisor_strategy;
mod system;
mod system_message;
mod system_state;

pub use actor::Actor;
pub use actor_context::ActorContext;
pub use actor_error::{ActorError, ActorErrorReason};
pub use actor_future::ActorFuture;
pub use actor_ref::{ActorRef, ActorRefSender};
pub use any_message::{AnyMessage, AnyMessageView};
pub use ask_response::AskResponse;
pub use child_ref::ChildRef;
pub use dispatcher::{DispatchExecutor, Dispatcher, DispatcherSender, InlineExecutor};
pub use mailbox::{EnqueueOutcome, Mailbox, MailboxMessage, MailboxOfferFuture, MailboxPollFuture};
pub use mailbox_policy::{MailboxCapacity, MailboxOverflowStrategy, MailboxPolicy};
pub use message_invoker::{MessageInvoker, MessageInvokerMiddleware, MessageInvokerPipeline};
pub use name_registry::{NameRegistry, NameRegistryError};
pub use pid::Pid;
pub use props::{ActorFactory, MailboxConfig, Props, SupervisorOptions};
pub use receive_state::ReceiveState;
pub use restart_statistics::RestartStatistics;
pub use send_error::SendError;
pub use spawn_error::SpawnError;
pub use supervisor_strategy::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind};
pub use system::ActorSystem;
pub use system_message::SystemMessage;
