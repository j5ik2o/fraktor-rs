#![deny(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_types, clippy::redundant_clone))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![cfg_attr(not(test), deny(clippy::redundant_clone))]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enums)]
#![deny(clippy::no_effect)]
#![deny(dropping_copy_types)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![deny(unreachable_pub)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(feature = "unsize", feature(unsize, coerce_unsized, dispatch_from_dyn))]
#![cfg_attr(feature = "unsize", allow(incomplete_features))]
//! Core utility collection.
//!
//! Provides fundamental data structures such as mailboxes, synchronization primitives,
//! and deadline-based processing intended for cross-runtime sharing, with `no_std` support.
//! By interacting with `actor-core` through this crate, we maintain unidirectional dependencies,
//! and each runtime only needs to satisfy the abstractions defined here with their own
//! implementations.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub use concurrent::{
  AsyncBarrier, AsyncBarrierBackend, CountDownLatch, CountDownLatchBackend, GuardHandle, Synchronized,
  SynchronizedMutexBackend, SynchronizedRw, SynchronizedRwBackend, WaitGroup, WaitGroupBackend,
};
pub use sync::{
  ArcShared, Flag, SendBound, Shared, SharedBound, SharedDyn, SharedFactory, SharedFn, StateCell, StaticRefShared,
};
pub use time::{
  ClockKind, DriftMonitor, DriftStatus, ManualClock, MonotonicClock, SchedulerCapacityProfile, SchedulerTickHandle,
  TickEvent, TickLease, TimerEntry, TimerEntryMode, TimerHandleId, TimerInstant, TimerWheel, TimerWheelConfig,
  TimerWheelError,
};
pub use timing::{
  DeadLineTimer, DeadLineTimerError, DeadLineTimerExpired, DeadLineTimerKey, DeadLineTimerKeyAllocator, DelayFuture,
  DelayProvider, DelayTrigger, ManualDelayProvider, TimerDeadLine,
};

/// Core collections shared across the Cellex runtimes.
pub mod collections;
mod concurrent;
/// Network utilities for URI parsing.
pub mod net;
pub mod runtime_toolbox;
/// Synchronization primitives and shared ownership abstractions.
pub mod sync;
pub mod time;
mod timing;
