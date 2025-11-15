//! Backend layer traits and supporting types for stack operations.

mod async_stack_backend;
mod push_outcome;
mod sync_stack_backend;
mod stack_error;
mod stack_overflow_policy;
mod sync_stack_async_adapter;
mod vec_stack_backend;

pub use async_stack_backend::AsyncStackBackend;
pub use push_outcome::PushOutcome;
pub use sync_stack_backend::SyncStackBackend;
pub use stack_error::StackError;
pub use stack_overflow_policy::StackOverflowPolicy;
pub use sync_stack_async_adapter::SyncStackAsyncAdapter;
pub use vec_stack_backend::VecStackBackend;
