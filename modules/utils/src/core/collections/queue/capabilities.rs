//! Capability traits describing queue usage contracts.

mod multi_producer;
mod queue_capability;
mod queue_capability_error;
mod queue_capability_registry;
mod queue_capability_set;
mod single_consumer;
mod single_producer;
mod supports_peek;

pub use multi_producer::MultiProducer;
pub use queue_capability::QueueCapability;
pub use queue_capability_error::QueueCapabilityError;
pub use queue_capability_registry::QueueCapabilityRegistry;
pub use queue_capability_set::QueueCapabilitySet;
pub use single_consumer::SingleConsumer;
pub use single_producer::SingleProducer;
pub use supports_peek::SupportsPeek;

mod base;
