//! Count-down latch primitives.

mod count_down_latch_backend;
mod count_down_latch_struct;

#[cfg(test)]
mod tests;

pub use count_down_latch_backend::CountDownLatchBackend;
pub use count_down_latch_struct::CountDownLatch;
