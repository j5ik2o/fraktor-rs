//! Shared ownership utilities.

mod shared_bound;
mod shared_dyn;
mod shared_trait;

pub use shared_bound::SharedBound;
pub(crate) use shared_dyn::SharedDyn;
pub use shared_trait::Shared;

#[cfg(test)]
mod tests;
