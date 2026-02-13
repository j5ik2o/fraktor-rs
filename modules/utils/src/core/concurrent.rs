//! Concurrency primitives: barriers, latches, synchronized wrappers, and wait groups.

/// Async-aware barrier implementation.
pub mod async_barrier;
/// Count-down latch for coordinating concurrent tasks.
pub mod count_down_latch;
/// Synchronized wrappers providing mutual-exclusion and read-write access.
pub mod synchronized;
/// Wait group for awaiting a set of concurrent operations.
pub mod wait_group;
