//! Message adapter support for typed actors.

mod adapt_message;
mod adapter_entry;
mod adapter_envelope;
mod adapter_error;
mod adapter_outcome;
mod adapter_payload;
mod builder;
mod registry;

pub(crate) use adapt_message::AdaptMessage;
pub(crate) use adapter_entry::AdapterEntry;
pub(crate) use adapter_envelope::AdapterEnvelope;
pub use adapter_error::AdapterError;
pub(crate) use adapter_outcome::AdapterOutcome;
pub use adapter_payload::AdapterPayload;
pub use builder::MessageAdapterBuilder;
pub use registry::MessageAdapterRegistry;
