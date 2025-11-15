//! Module consolidating deadline and delay abstractions shared across runtimes.

mod dead_line_timer;
mod delay;

pub use dead_line_timer::{
  DeadLineTimer, DeadLineTimerError, DeadLineTimerExpired, DeadLineTimerKey, DeadLineTimerKeyAllocator, TimerDeadLine,
};
pub use delay::{DelayFuture, DelayProvider, DelayTrigger, ManualDelayProvider};
