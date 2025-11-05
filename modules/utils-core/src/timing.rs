//! Module that consolidates timer-related abstractions.
//!
//! Re-exports minimal APIs referenced from core for common use by time-triggered features such as
//! `ReceiveTimeout`.

mod dead_line_timer;

pub use dead_line_timer::{
  DeadLineTimer, DeadLineTimerError, DeadLineTimerExpired, DeadLineTimerKey, DeadLineTimerKeyAllocator, TimerDeadLine,
};
