//! Stack abstractions rebuilt for the v2 collections layer.

mod async_stack;
mod async_stack_shared;
pub mod backend;
mod sync_stack;
mod sync_stack_shared;
#[cfg(test)]
mod tests;

pub use async_stack::AsyncStack;
pub use async_stack_shared::AsyncStackShared;
pub use backend::{
  AsyncStackBackend, PushOutcome, StackError, StackOverflowPolicy, SyncStackAsyncAdapter, SyncStackBackend,
  VecStackBackend,
};
pub use sync_stack::SyncStack;
pub use sync_stack_shared::SyncStackShared;

use crate::sync::{async_mutex_like::SpinAsyncMutex, sync_mutex_like::SpinSyncMutex};

/// Default shared stack alias backed by [`VecStackBackend`].
pub type SharedVecStack<T> = SyncStackShared<T, VecStackBackend<T>, SpinSyncMutex<SyncStack<T, VecStackBackend<T>>>>;

/// Default async shared stack alias backed by [`VecStackBackend`] via the sync adapter.
pub type AsyncSharedVecStack<T> = AsyncStackShared<
  T,
  SyncStackAsyncAdapter<T, VecStackBackend<T>>,
  SpinAsyncMutex<AsyncStack<T, SyncStackAsyncAdapter<T, VecStackBackend<T>>>>,
>;
