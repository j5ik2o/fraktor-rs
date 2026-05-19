//! DSL package for typed actor development.
//!
//! Mirrors Pekko's `scaladsl` package with high-level building blocks:
//! behaviors, stash, timers, supervisors, ask patterns, status replies, and routing.

/// Typed routing package for routers, builders, and resizers.
pub mod routing;

mod abstract_behavior;
mod ask_pattern;
mod behaviors;
mod failure_handler;
mod fsm_builder;
/// Intermediate receive builder for typed behaviors.
mod receive;
mod stash_buffer;
mod status_reply;
mod status_reply_error;
mod supervise;
mod timer_key;
mod timer_scheduler;
mod typed_ask_error;
mod typed_ask_future;
mod typed_ask_response;

pub use abstract_behavior::AbstractBehavior;
pub use ask_pattern::AskPattern;
pub use behaviors::Behaviors;
pub use failure_handler::FailureHandler;
pub use fsm_builder::FsmBuilder;
pub use receive::Receive;
pub use stash_buffer::StashBuffer;
pub use status_reply::StatusReply;
pub use status_reply_error::StatusReplyError;
pub use supervise::Supervise;
pub use timer_key::TimerKey;
pub use timer_scheduler::{TimerScheduler, TimerSchedulerShared};
pub use typed_ask_error::TypedAskError;
pub use typed_ask_future::TypedAskFuture;
pub use typed_ask_response::TypedAskResponse;
