//! std-only extension for LocalClusterProvider.
//!
//! This module provides Transport event auto-detection functionality
//! that is only available in std environments.

use fraktor_actor_core_rs::event::stream::{
  ClassifierKey, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
  RemotingLifecycleEvent, subscriber_handle,
};
use fraktor_cluster_core_rs::core::cluster_provider::{
  LocalClusterProvider, LocalClusterProviderShared, LocalClusterProviderWeak,
};
use fraktor_utils_core_rs::core::sync::SharedAccess;

#[cfg(test)]
mod tests;

/// Subscribes to remoting lifecycle events for automatic topology updates.
///
/// This function registers a subscriber to the event stream that listens for
/// `RemotingLifecycleEvent::Connected` and `Quarantined` events, automatically
/// triggering `TopologyUpdated` events when nodes join or leave.
///
/// **Note**: This function is only available in std environments.
pub fn subscribe_remoting_events(provider: &LocalClusterProviderShared) -> EventStreamSubscription {
  struct RemotingEventHandler {
    provider: LocalClusterProviderWeak,
  }

  impl EventStreamSubscriber for RemotingEventHandler {
    fn on_event(&mut self, event: &EventStreamEvent) {
      let Some(provider) = self.provider.upgrade() else {
        return;
      };
      if let EventStreamEvent::RemotingLifecycle(lifecycle_event) = event {
        // 起動前は無視
        if !provider.with_read(|p| p.is_started()) {
          return;
        }
        match lifecycle_event {
          | RemotingLifecycleEvent::Connected { authority, .. } => {
            provider.with_write(|p| p.handle_connected(authority));
          },
          | RemotingLifecycleEvent::Quarantined { authority, .. } => {
            provider.with_write(|p| p.handle_quarantined(authority));
          },
          | _ => {},
        }
      }
    }
  }

  // event_stream への参照を取得
  let event_stream = provider.with_read(|p| p.event_stream().clone());
  let handler = RemotingEventHandler { provider: provider.downgrade() };
  let subscriber: EventStreamSubscriberShared = subscriber_handle(handler);
  event_stream.subscribe_with_key(ClassifierKey::RemotingLifecycle, &subscriber)
}

/// Creates a shared, thread-safe LocalClusterProvider wrapped in a mutex.
pub fn wrap_local_cluster_provider(provider: LocalClusterProvider) -> LocalClusterProviderShared {
  LocalClusterProviderShared::new(provider)
}
