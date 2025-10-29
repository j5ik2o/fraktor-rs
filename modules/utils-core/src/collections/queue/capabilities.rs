//! Capability traits describing queue usage contracts.

mod multi_producer;
mod single_consumer;
mod single_producer;
mod supports_peek;

pub use multi_producer::MultiProducer;
pub use single_consumer::SingleConsumer;
pub use single_producer::SingleProducer;
pub use supports_peek::SupportsPeek;

mod impls;
