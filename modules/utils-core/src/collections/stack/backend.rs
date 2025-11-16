//! Backend layer traits and supporting types for stack operations.

mod async_stack_backend;
mod async_stack_backend_internal;
mod push_outcome;
mod stack_error;
mod stack_overflow_policy;
mod sync_stack_async_adapter;
mod sync_stack_backend;
mod sync_stack_backend_internal;
mod vec_stack_backend;

pub use async_stack_backend::AsyncStackBackend;
pub(crate) use async_stack_backend_internal::AsyncStackBackendInternal;
pub use push_outcome::PushOutcome;
pub use stack_error::StackError;
pub use stack_overflow_policy::StackOverflowPolicy;
pub use sync_stack_async_adapter::SyncStackAsyncAdapter;
pub use sync_stack_backend::SyncStackBackend;
pub(crate) use sync_stack_backend_internal::SyncStackBackendInternal;
pub use vec_stack_backend::VecStackBackend;
