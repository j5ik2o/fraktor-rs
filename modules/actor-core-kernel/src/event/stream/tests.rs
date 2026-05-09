#![cfg(test)]

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::{SharedLock, SpinSyncMutex};

use super::{EventStreamSubscriber, EventStreamSubscriberShared};

#[must_use]
pub(crate) fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  EventStreamSubscriberShared::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<_>>(Box::new(subscriber)))
}
