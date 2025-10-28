//! Storage layer abstractions for stack backends.

mod stack_storage;
mod vec_stack_storage;

pub use stack_storage::StackStorage;
pub use vec_stack_storage::VecStackStorage;
