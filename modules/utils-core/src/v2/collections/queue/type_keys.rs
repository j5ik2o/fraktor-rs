//! Type-level markers describing queue variants.

mod fifo_key;
mod mpsc_key;
mod priority_key;
mod spsc_key;
mod type_key;

pub use fifo_key::FifoKey;
pub use mpsc_key::MpscKey;
pub use priority_key::PriorityKey;
pub use spsc_key::SpscKey;
pub use type_key::TypeKey;
