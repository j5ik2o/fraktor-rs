//! Count-down latch primitives.

mod count_down_latch_backend;
mod count_down_latch_struct;

pub use count_down_latch_backend::CountDownLatchBackend;
pub use count_down_latch_struct::CountDownLatch;
