//! Shared wait primitives used by async collection adapters.

mod error;
mod handle_shared;
mod node;
mod queue;

pub use error::WaitError;
pub use handle_shared::WaitShared;
pub use queue::WaitQueue;
