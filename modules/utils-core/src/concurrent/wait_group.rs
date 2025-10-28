//! Wait-group primitives.

mod wait_group_backend;
mod wait_group_struct;

pub use wait_group_backend::WaitGroupBackend;
pub use wait_group_struct::WaitGroup;
