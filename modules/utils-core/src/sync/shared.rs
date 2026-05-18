//! Shared ownership utilities.

mod shared_bound;
mod shared_trait;

pub use shared_bound::SharedBound;
pub use shared_trait::Shared;

#[cfg(test)]
#[path = "shared_test.rs"]
mod tests;
