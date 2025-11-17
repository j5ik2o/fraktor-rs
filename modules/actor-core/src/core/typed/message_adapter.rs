//! Message adapter support for typed actors.

mod adapt_message;
mod adapter_entry;
mod adapter_envelope;
mod adapter_error;
mod adapter_failure;
mod adapter_failure_event;
mod adapter_lifecycle_state;
mod adapter_outcome;
mod adapter_payload;
mod adapter_ref_handle;
mod adapter_ref_handle_id;
mod adapter_ref_sender;
mod registry;

pub use adapt_message::AdaptMessage;
pub use adapter_entry::AdapterEntry;
pub use adapter_envelope::AdapterEnvelope;
pub use adapter_error::AdapterError;
pub use adapter_failure::AdapterFailure;
pub use adapter_failure_event::AdapterFailureEvent;
pub use adapter_lifecycle_state::AdapterLifecycleState;
pub use adapter_outcome::AdapterOutcome;
pub use adapter_payload::AdapterPayload;
pub use adapter_ref_handle::AdapterRefHandle;
pub use adapter_ref_handle_id::AdapterRefHandleId;
pub use adapter_ref_sender::AdapterRefSender;
pub use registry::MessageAdapterRegistry;
