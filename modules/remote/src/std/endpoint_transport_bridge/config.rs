//! Re-export of [`crate::core::remoting_extension::EndpointBridgeConfig`].
//!
//! The bridge config is owned by `core` so the dependency direction stays
//! `std → core`. This module exists only to keep the existing internal
//! import paths (`super::EndpointTransportBridgeConfig`) working.

pub use crate::core::remoting_extension::endpoint_bridge::EndpointBridgeConfig as EndpointTransportBridgeConfig;
