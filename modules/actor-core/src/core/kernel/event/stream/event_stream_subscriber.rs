//! Trait implemented by event stream observers.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock};

use crate::core::kernel::{event::stream::EventStreamEvent, system::lock_provider::ActorLockProvider};

/// Shared subscriber handle guarded by the workspace's compile-time selected
/// default lock driver.
///
/// `EventStreamSubscriberShared` is exposed as a `SharedLock` so that:
///
/// - the default construction path (`subscriber_handle`) goes through [`SharedLock::new`] and uses
///   the cfg-selected default driver, and
/// - test/diagnostic builds can swap in a different driver (for example `DebugSpinSyncMutex` for
///   re-entry detection) by routing through [`subscriber_handle_with_lock_provider`].
pub type EventStreamSubscriberShared = SharedLock<Box<dyn EventStreamSubscriber>>;

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Invoked for every published event.
  fn on_event(&mut self, event: &EventStreamEvent);
}

/// Wraps the subscriber into a mutex-protected shared handle backed by the
/// workspace's compile-time selected default lock driver.
#[must_use]
pub fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  SharedLock::new(Box::new(subscriber) as Box<dyn EventStreamSubscriber>)
}

/// Wraps the subscriber using an optional [`ActorLockProvider`] override.
///
/// `provider` is an [`Option`] so that callers can pass the result of
/// [`SystemState::lock_provider`](crate::core::kernel::system::state::SystemState::lock_provider)
/// directly without first matching it. When `Some`, the provider's
/// [`create_event_stream_subscriber_shared`](ActorLockProvider::create_event_stream_subscriber_shared)
/// hook is invoked, which is the route that
/// `DebugActorLockProvider` / `StdActorLockProvider` /
/// `ParkingLotActorLockProvider` use to swap the lock driver. When `None`,
/// the default `SharedLock::new` path is used.
#[must_use]
pub fn subscriber_handle_with_lock_provider(
  provider: &Option<ArcShared<dyn ActorLockProvider>>,
  subscriber: impl EventStreamSubscriber,
) -> EventStreamSubscriberShared {
  let boxed: Box<dyn EventStreamSubscriber> = Box::new(subscriber);
  match provider {
    | Some(provider) => provider.create_event_stream_subscriber_shared(boxed),
    | None => SharedLock::new(boxed),
  }
}
