//! Typed configuration for the remote subsystem.

#[cfg(test)]
mod tests;

mod remote_config;

pub use remote_config::RemoteConfig;
pub(crate) use remote_config::{
  DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE, DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE,
  DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER,
};
