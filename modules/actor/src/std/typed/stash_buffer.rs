//! Std alias for typed stash buffer helpers.

use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Type alias for the typed stash buffer bound to `StdToolbox`.
pub type StashBuffer<M> = crate::core::typed::StashBufferGeneric<M, StdToolbox>;
