#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(test), no_std)]
#![allow(clippy::module_inception)]

//! Remoting facilities for the fraktor actor runtime.

extern crate alloc;

/// Core remoting facilities.
pub mod core;
/// Standard library implementation.
#[cfg(feature = "std")]
mod std;

pub use core::{
  InboundEnvelope, RemoteActorRefProvider, RemoteActorRefProviderSetup, RemoteWatcherDaemon, RemoteWatcherMessage,
  RemotingBackpressureListener, RemotingConnectionSnapshot, RemotingControl, RemotingControlHandle, RemotingError,
  RemotingExtension, RemotingExtensionConfig, RemotingExtensionId,
};
