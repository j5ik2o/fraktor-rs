//! Shared wait primitives used by async collection adapters.

mod error;
mod handle_shared;
mod node;
mod queue;

use crate::core::{
  runtime_toolbox::{NoStdToolbox, ToolboxMutex},
  sync::ArcShared,
};

pub(crate) type WaitNodeShared<E, TB = NoStdToolbox> = ArcShared<ToolboxMutex<node::WaitNode<E>, TB>>;

pub use error::WaitError;
pub use handle_shared::WaitShared;
pub use queue::WaitQueue;
