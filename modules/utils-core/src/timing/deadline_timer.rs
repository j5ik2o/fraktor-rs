//! Common deadline timer types.

mod deadline_timer_error;
mod deadline_timer_expired;
mod deadline_timer_key;
mod deadline_timer_key_allocator;
mod deadline_timer_trait;
mod timer_deadline;

pub use deadline_timer_error::DeadlineTimerError;
pub use deadline_timer_expired::DeadlineTimerExpired;
pub use deadline_timer_key::DeadlineTimerKey;
pub use deadline_timer_key_allocator::DeadlineTimerKeyAllocator;
pub use deadline_timer_trait::DeadlineTimer;
pub use timer_deadline::TimerDeadline;

#[cfg(test)]
mod tests;
