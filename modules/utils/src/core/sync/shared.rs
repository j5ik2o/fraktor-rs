//! Shared ownership utilities.

mod send_bound;
mod shared_bound;
mod shared_dyn;
mod shared_trait;

#[allow(unused_imports)]
pub(crate) use send_bound::SendBound;
pub use shared_bound::SharedBound;
pub(crate) use shared_dyn::SharedDyn;
pub use shared_trait::Shared;

#[cfg(test)]
mod tests;
