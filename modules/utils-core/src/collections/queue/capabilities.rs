//! Capability traits describing queue usage contracts.

mod multi_producer;
mod registry;
mod single_consumer;
mod single_producer;
mod supports_peek;

pub use multi_producer::MultiProducer;
pub use registry::{QueueCapability, QueueCapabilityError, QueueCapabilityRegistry, QueueCapabilitySet};
pub use single_consumer::SingleConsumer;
pub use single_producer::SingleProducer;
pub use supports_peek::SupportsPeek;

mod base;
