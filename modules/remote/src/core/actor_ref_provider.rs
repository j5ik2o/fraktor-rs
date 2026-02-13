//! Actor reference providers for remote communication.
//!
//! This module provides implementations of [`ActorRefProvider`] for different
//! transport types (Loopback, Remote TCP, Tokio TCP) and their installers.

mod loopback_router;
mod remote_error;

/// Loopback actor reference provider and installer.
pub mod loopback;
/// Remote actor reference provider and installer.
pub mod remote;
/// Tokio TCP actor reference provider and installer.
pub mod tokio;

pub(crate) use loopback_router::unregister_endpoint;
