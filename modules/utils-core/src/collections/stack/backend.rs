//! Backend layer traits and supporting types for stack operations.

mod async_stack_backend;
mod push_outcome;
mod stack_backend;
mod stack_error;
mod stack_overflow_policy;
mod sync_adapter_stack_backend;
mod vec_stack_backend;

pub use async_stack_backend::AsyncStackBackend;
pub use push_outcome::PushOutcome;
pub use stack_backend::StackBackend;
pub use stack_error::StackError;
pub use stack_overflow_policy::StackOverflowPolicy;
pub use sync_adapter_stack_backend::SyncAdapterStackBackend;
pub use vec_stack_backend::VecStackBackend;
