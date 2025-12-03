//! Futures package.
//!
//! This module contains Future integration.

mod actor_future;
mod actor_future_listener;

pub use actor_future::{ActorFuture, ActorFutureShared};
pub use actor_future_listener::ActorFutureListener;
