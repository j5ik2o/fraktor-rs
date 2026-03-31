//! DSL package for typed actor development.
//!
//! Mirrors Pekko's `scaladsl` package with high-level building blocks:
//! behaviors, stash, timers, supervisors, ask patterns, status replies, and routing.

/// Typed routing package for routers, builders, and resizers.
pub mod routing;

mod behaviors;
mod failure_handler;
mod fsm_builder;
mod stash_buffer;
mod status_reply;
mod status_reply_error;
mod supervise;
mod timer_key;
mod timer_scheduler;
mod typed_ask_error;
mod typed_ask_future;
mod typed_ask_response;

pub use behaviors::Behaviors;
pub use failure_handler::FailureHandler;
pub use fsm_builder::FsmBuilder;
pub use stash_buffer::StashBuffer;
pub use status_reply::StatusReply;
pub use status_reply_error::StatusReplyError;
pub use supervise::Supervise;
pub use timer_key::TimerKey;
pub use timer_scheduler::{TimerScheduler, TimerSchedulerShared};
pub use typed_ask_error::TypedAskError;
pub use typed_ask_future::TypedAskFuture;
pub use typed_ask_response::TypedAskResponse;
