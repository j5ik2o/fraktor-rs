//! std-only extension for LocalClusterProviderGeneric.
//!
//! This module provides Transport event auto-detection functionality
//! that is only available in std environments.

use fraktor_actor_rs::core::event::stream::{
  EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscriptionGeneric,
  RemotingLifecycleEvent, subscriber_handle,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use crate::core::cluster_provider::{LocalClusterProviderGeneric, LocalClusterProviderSharedGeneric};

/// Subscribes to remoting lifecycle events for automatic topology updates.
///
/// This function registers a subscriber to the event stream that listens for
/// `RemotingLifecycleEvent::Connected` and `Quarantined` events, automatically
/// triggering `TopologyUpdated` events when nodes join or leave.
///
/// **Note**: This function is only available in std environments with `StdToolbox`.
pub fn subscribe_remoting_events<TB>(provider: &LocalClusterProviderSharedGeneric<TB>)
where
  TB: RuntimeToolbox + 'static, {
  struct RemotingEventHandler<TB: RuntimeToolbox + 'static> {
    provider: LocalClusterProviderSharedGeneric<TB>,
  }

  impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for RemotingEventHandler<TB> {
    fn on_event(&mut self, event: &EventStreamEvent<TB>) {
      if let EventStreamEvent::Extension { name, payload } = event {
        if name == "remoting" {
          // 起動前は無視
          if !self.provider.with_read(|p| p.is_started()) {
            return;
          }
          if let Some(lifecycle_event) = payload.payload().downcast_ref::<RemotingLifecycleEvent>() {
            match lifecycle_event {
              | RemotingLifecycleEvent::Connected { authority, .. } => {
                self.provider.with_write(|p| p.handle_connected(authority));
              },
              | RemotingLifecycleEvent::Quarantined { authority, .. } => {
                self.provider.with_write(|p| p.handle_quarantined(authority));
              },
              | _ => {},
            }
          }
        }
      }
    }
  }

  // event_stream への参照を取得
  let event_stream = provider.with_read(|p| p.event_stream().clone());
  let handler = RemotingEventHandler { provider: provider.clone() };
  let subscriber: EventStreamSubscriberShared<TB> = subscriber_handle(handler);
  let _subscription: EventStreamSubscriptionGeneric<TB> = event_stream.subscribe(&subscriber);
  // Note: subscription は provider のライフタイムに依存するので、
  // provider がドロップされるまで有効
}

/// Creates a shared, thread-safe LocalClusterProviderGeneric wrapped in a mutex.
pub fn wrap_local_cluster_provider<TB>(
  provider: LocalClusterProviderGeneric<TB>,
) -> LocalClusterProviderSharedGeneric<TB>
where
  TB: RuntimeToolbox + 'static, {
  LocalClusterProviderSharedGeneric::new(provider)
}
