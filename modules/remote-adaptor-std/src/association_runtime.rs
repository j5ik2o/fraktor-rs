//! Tokio-based runtime that drives the pure `Association` state machine.
//!
//! This module materialises the side-effects emitted by
//! [`fraktor_remote_core_rs::association::Association`] (`StartHandshake`,
//! `SendEnvelopes`, `DiscardEnvelopes`, `PublishLifecycle`) on top of a real
//! TCP transport. The decomposition follows Apache Pekko Artery's runtime
//! split:
//!
//! - [`association_shared::AssociationShared`] is the `AShared` wrapper that lets multiple tokio
//!   tasks share a single `Association` while still honouring the `&mut self` contract of the pure
//!   state machine.
//! - [`association_registry::AssociationRegistry`] keeps a `BTreeMap` of per-remote
//!   `AssociationShared` handles.
//! - [`outbound_loop::run_outbound_loop`] drains
//!   [`fraktor_remote_core_rs::association::Association::next_outbound`] and forwards envelopes to
//!   the transport.
//! - [`inbound_dispatch::run_inbound_dispatch`] reads inbound frames from the TCP layer and
//!   dispatches them to the matching `Association`.
//! - [`handshake_driver::HandshakeDriver`] arms a `tokio::time::sleep` to call
//!   `Association::handshake_timed_out` when the deadline expires.
//! - [`system_message_delivery::SystemMessageDeliveryState`] holds the per-association ack-based
//!   redelivery bookkeeping (sequence number, pending window, retransmit deadline).

#[cfg(test)]
mod tests;

mod association_registry;
mod association_shared;
mod handshake_driver;
mod inbound_dispatch;
mod outbound_loop;
mod system_message_delivery;

pub use association_registry::AssociationRegistry;
pub use association_shared::AssociationShared;
pub use handshake_driver::HandshakeDriver;
pub use inbound_dispatch::run_inbound_dispatch;
pub use outbound_loop::run_outbound_loop;
pub use system_message_delivery::SystemMessageDeliveryState;
