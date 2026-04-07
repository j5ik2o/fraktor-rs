//! `core::task::Waker` implementation that re-schedules a mailbox via the dispatcher.
//!
//! `DispatcherWaker` is the no_std-friendly waker installed by `MailboxOfferFuture`.
//! When a future is woken, it asks the dispatcher to re-register the mailbox so the
//! drain loop can run again on the next executor pass.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::task::{RawWaker, RawWakerVTable, Waker};

use fraktor_utils_rs::core::sync::ArcShared;

use super::message_dispatcher_shared::MessageDispatcherShared;
use crate::core::kernel::dispatch::mailbox::Mailbox;

/// State payload referenced by the [`RawWaker`] data pointer.
struct WakerState {
  dispatcher: MessageDispatcherShared,
  mailbox:    ArcShared<Mailbox>,
}

/// Builds a `core::task::Waker` that re-schedules `mailbox` through `dispatcher`.
#[must_use]
pub fn dispatcher_waker(dispatcher: MessageDispatcherShared, mailbox: ArcShared<Mailbox>) -> Waker {
  let state = Box::new(WakerState { dispatcher, mailbox });
  let raw = RawWaker::new(Box::into_raw(state).cast::<()>(), &VTABLE);
  // SAFETY: `raw` is constructed with vtable functions that maintain the
  // ownership invariants documented on `RawWakerVTable`. Specifically:
  //  - `clone_waker` clones the boxed state by re-issuing a new RawWaker that owns its own copy of
  //    the inner ArcShared/MessageDispatcherShared.
  //  - `wake` and `wake_by_ref` only invoke the dispatcher; the data pointer is never deallocated
  //    unless `wake` consumes it.
  //  - `drop_waker` boxes the data pointer back into a `Box<WakerState>` and drops it.
  unsafe { Waker::from_raw(raw) }
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, wake_waker, wake_by_ref_waker, drop_waker);

unsafe fn clone_waker(data: *const ()) -> RawWaker {
  // SAFETY: `data` originated from `Box::into_raw` of a `Box<WakerState>` and is
  // valid for the lifetime of the surrounding `Waker`. We borrow it briefly to
  // clone the inner state.
  let state = unsafe { &*data.cast::<WakerState>() };
  let cloned = Box::new(WakerState { dispatcher: state.dispatcher.clone(), mailbox: state.mailbox.clone() });
  RawWaker::new(Box::into_raw(cloned).cast::<()>(), &VTABLE)
}

unsafe fn wake_waker(data: *const ()) {
  // SAFETY: `data` was produced by `Box::into_raw` and is now consumed.
  let boxed = unsafe { Box::from_raw(data.cast::<WakerState>().cast_mut()) };
  // `DispatcherWaker` is installed by `MailboxOfferFuture` for user-message
  // backpressure wake-ups, so the hints must describe "a user message is
  // pending", not a system message. Inverting these hints would let the
  // wake-up pass `request_schedule`'s suspended-mailbox guard even when no
  // user-message work is pending, which would stall backpressure completion.
  let _scheduled = boxed.dispatcher.register_for_execution(&boxed.mailbox, true, false);
}

unsafe fn wake_by_ref_waker(data: *const ()) {
  // SAFETY: `data` was produced by `Box::into_raw` and remains live for the
  // entire lifetime of the surrounding `Waker`.
  let state = unsafe { &*data.cast::<WakerState>() };
  let _scheduled = state.dispatcher.register_for_execution(&state.mailbox, true, false);
}

unsafe fn drop_waker(data: *const ()) {
  // SAFETY: `data` was produced by `Box::into_raw` and is no longer used after this drop.
  let _ = unsafe { Box::from_raw(data.cast::<WakerState>().cast_mut()) };
}
