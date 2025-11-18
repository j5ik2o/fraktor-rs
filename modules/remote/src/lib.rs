#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(test), no_std)]

//! Remoting facilities for the fraktor actor runtime.

extern crate alloc;

mod core;
mod std;

pub use core::{
  backpressure_listener::RemotingBackpressureListener,
  endpoint_writer::{EndpointWriter, OutboundEnvelope, RemotingEnvelope},
  remoting_connection_snapshot::RemotingConnectionSnapshot,
  remoting_control::RemotingControl,
  remoting_control_handle::RemotingControlHandle,
  remoting_error::RemotingError,
  remoting_extension::RemotingExtension,
  remoting_extension_config::RemotingExtensionConfig,
  remoting_extension_id::RemotingExtensionId,
};
