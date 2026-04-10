#![cfg(test)]

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

use super::{EventStreamSubscriber, EventStreamSubscriberShared};

#[must_use]
pub(crate) fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  SharedLock::new_with_driver::<SpinSyncMutex<_>>(Box::new(subscriber))
}
