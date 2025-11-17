//! Wait-group primitives.

mod wait_group_backend;
mod wait_group_struct;

#[cfg(test)]
mod tests;

pub use wait_group_backend::WaitGroupBackend;
pub use wait_group_struct::WaitGroup;
