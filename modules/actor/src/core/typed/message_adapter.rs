//! Message adapter support for typed actors.

mod adapt_message;
mod adapter_entry;
mod adapter_envelope;
mod adapter_error;
mod adapter_lifecycle_state;
mod adapter_outcome;
mod adapter_payload;
mod adapter_ref_handle;
mod adapter_ref_sender;
mod builder;
mod registry;

pub(crate) type AdapterRefHandleId = u64;

pub(crate) use adapt_message::AdaptMessage;
pub(crate) use adapter_entry::AdapterEntry;
pub(crate) use adapter_envelope::AdapterEnvelope;
pub use adapter_error::AdapterError;
pub(crate) use adapter_lifecycle_state::AdapterLifecycleState;
pub(crate) use adapter_outcome::AdapterOutcome;
pub use adapter_payload::AdapterPayload;
pub(crate) use adapter_ref_handle::AdapterRefHandle;
pub(crate) use adapter_ref_sender::AdapterRefSender;
pub use builder::MessageAdapterBuilderGeneric;
pub use registry::MessageAdapterRegistry;
