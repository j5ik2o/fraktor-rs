//! Typed configuration for the remote subsystem.

#[cfg(test)]
#[path = "config_test.rs"]
mod tests;

mod large_message_destination_pattern;
mod large_message_destinations;
mod remote_compression_config;
mod remote_config;

pub use large_message_destination_pattern::LargeMessageDestinationPattern;
pub use large_message_destinations::LargeMessageDestinations;
pub use remote_compression_config::RemoteCompressionConfig;
pub use remote_config::RemoteConfig;
pub(crate) use remote_config::{
  DEFAULT_ACK_RECEIVE_WINDOW, DEFAULT_ACK_SEND_WINDOW, DEFAULT_HANDSHAKE_TIMEOUT, DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE,
  DEFAULT_OUTBOUND_LARGE_MESSAGE_QUEUE_SIZE, DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE,
  DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER,
};
