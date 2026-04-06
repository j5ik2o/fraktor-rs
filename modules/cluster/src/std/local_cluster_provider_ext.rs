//! std-only extension for LocalClusterProvider.
//!
//! This module provides Transport event auto-detection functionality
//! that is only available in std environments.

use fraktor_actor_core_rs::core::kernel::event::stream::{
  EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
  RemotingLifecycleEvent, subscriber_handle,
};
use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::cluster_provider::{LocalClusterProvider, LocalClusterProviderShared};

/// Subscribes to remoting lifecycle events for automatic topology updates.
///
/// This function registers a subscriber to the event stream that listens for
/// `RemotingLifecycleEvent::Connected` and `Quarantined` events, automatically
/// triggering `TopologyUpdated` events when nodes join or leave.
///
/// **Note**: This function is only available in std environments.
pub fn subscribe_remoting_events(provider: &LocalClusterProviderShared) {
  struct RemotingEventHandler {
    provider: LocalClusterProviderShared,
  }

  impl EventStreamSubscriber for RemotingEventHandler {
    fn on_event(&mut self, event: &EventStreamEvent) {
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
  let subscriber: EventStreamSubscriberShared = subscriber_handle(handler);
  let _subscription: EventStreamSubscription = event_stream.subscribe(&subscriber);
  // Note: subscription は provider のライフタイムに依存するので、
  // provider がドロップされるまで有効
}

/// Creates a shared, thread-safe LocalClusterProvider wrapped in a mutex.
pub fn wrap_local_cluster_provider(provider: LocalClusterProvider) -> LocalClusterProviderShared {
  LocalClusterProviderShared::new(provider)
}
