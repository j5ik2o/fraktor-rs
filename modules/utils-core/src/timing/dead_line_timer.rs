//! Common deadline timer types.

mod dead_line_timer_error;
mod dead_line_timer_expired;
mod dead_line_timer_key;
mod dead_line_timer_key_allocator;
mod dead_line_timer_trait;
mod timer_dead_line;

pub use dead_line_timer_error::DeadLineTimerError;
pub use dead_line_timer_expired::DeadLineTimerExpired;
pub use dead_line_timer_key::DeadLineTimerKey;
pub use dead_line_timer_key_allocator::DeadLineTimerKeyAllocator;
pub use dead_line_timer_trait::DeadLineTimer;
pub use timer_dead_line::TimerDeadLine;

#[cfg(test)]
mod tests;
