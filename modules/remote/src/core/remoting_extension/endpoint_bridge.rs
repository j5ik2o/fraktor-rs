//! Trait abstractions for spawning the endpoint transport bridge.
//!
//! `core` defines the contract; `std` (or other adapters) provide the
//! concrete implementation. This keeps the dependency direction `std → core`
//! and avoids any reverse dependency from `core` into `std`.

mod config;
mod factory;
mod handle;

pub use config::EndpointBridgeConfig;
pub use factory::EndpointBridgeFactory;
pub use handle::EndpointBridgeHandle;
