#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![deny(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_types))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![deny(clippy::redundant_clone)]
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
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
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
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::redundant_clone))]
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
//! Core actor runtime primitives.

extern crate alloc;

#[cfg(any(feature = "std", test))]
extern crate std;

mod actor;
mod actor_context;
mod actor_error;
mod actor_future;
mod actor_ref;
mod any_message;
mod any_owned_message;
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
mod supervisor_strategy;
mod system;

pub use actor::Actor;
pub use actor_context::ActorContext;
pub use actor_error::ActorError;
pub use actor_future::ActorFuture;
pub use actor_ref::ActorRef;
pub use any_message::AnyMessage;
pub use any_owned_message::AnyOwnedMessage;
pub use child_ref::ChildRef;
pub use dispatcher::Dispatcher;
pub use mailbox::{Mailbox, MailboxEnqueue, MailboxError};
pub use mailbox_policy::MailboxPolicy;
pub use message_invoker::MessageInvoker;
pub use name_registry::NameRegistry;
pub use pid::Pid;
pub use props::{MailboxCapacity, MailboxConfig, Props, SupervisorOptions};
pub use receive_state::ReceiveState;
pub use restart_statistics::RestartStatistics;
pub use send_error::SendError;
pub use supervisor_strategy::SupervisorStrategy;
pub use system::ActorSystem;
