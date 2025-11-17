//! Async barrier primitives.

mod async_barrier_backend;
mod async_barrier_struct;

#[cfg(test)]
mod tests;

pub use async_barrier_backend::AsyncBarrierBackend;
pub use async_barrier_struct::AsyncBarrier;
