//! Runtime-owned message adapter lifecycle handles.

mod adapter_lifecycle_state;
mod adapter_ref_handle;
mod adapter_ref_sender;
mod message_adapter_lease;
mod message_adapter_ref;

/// Identifier assigned to a message adapter handle owned by an actor cell.
pub(crate) type AdapterRefHandleId = u64;

pub(crate) use adapter_lifecycle_state::AdapterLifecycleState;
pub(crate) use adapter_ref_handle::AdapterRefHandle;
pub(crate) use adapter_ref_sender::AdapterRefSender;
pub use message_adapter_lease::MessageAdapterLease;
pub use message_adapter_ref::MessageAdapterRef;
