//! Shared wait primitives used by async collection adapters.

mod handle_shared;
mod node;
mod queue;

pub use handle_shared::WaitShared;
pub use queue::WaitQueue;
