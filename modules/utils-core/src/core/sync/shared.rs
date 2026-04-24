//! Shared ownership utilities.

mod shared_bound;
mod shared_trait;

pub use shared_bound::SharedBound;
pub use shared_trait::Shared;

#[cfg(test)]
mod tests;
