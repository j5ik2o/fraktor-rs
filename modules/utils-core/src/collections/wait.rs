//! Shared wait primitives used by async collection adapters.

mod handle;
mod node;
mod queue;

pub use handle::WaitHandle;
pub use queue::WaitQueue;
