//! Shared wait primitives used by async collection adapters.

mod error;
mod handle_shared;
mod node;
mod node_shared;
mod queue;

pub use error::WaitError;
pub use handle_shared::WaitShared;
pub(crate) use node_shared::WaitNodeShared;
pub use queue::WaitQueue;
