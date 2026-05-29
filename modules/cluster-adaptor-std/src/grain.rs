//! Std helpers for virtual actor grain APIs.

mod grain_std_call_options;

pub use grain_std_call_options::{call_options_with_retry, call_options_with_timeout, default_grain_call_options};
