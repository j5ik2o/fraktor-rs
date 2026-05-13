//! Address primitives for remote actor systems.
//!
//! These types model the Pekko Artery `Address` / `UniqueAddress` pair and the actor
//! path URI scheme, expressed without any `std` or transport dependency.

#[cfg(test)]
#[path = "address_test.rs"]
mod tests;

mod base;
mod remote_node_id;
mod scheme;
mod unique_address;

pub use base::Address;
pub use remote_node_id::RemoteNodeId;
pub use scheme::ActorPathScheme;
pub use unique_address::UniqueAddress;
