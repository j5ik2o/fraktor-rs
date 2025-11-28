//! std-only extension for LocalClusterProvider.
//!
//! This module provides Transport event auto-detection functionality
//! that is only available in std environments.

use fraktor_actor_rs::core::event_stream::{
  EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriberShared,
  EventStreamSubscriptionGeneric, RemotingLifecycleEvent, subscriber_handle,
};
use fraktor_utils_rs::{
  core::{
    runtime_toolbox::{SyncMutexFamily, ToolboxMutex},
    sync::ArcShared,
  },
  std::runtime_toolbox::StdToolbox,
};

use crate::core::LocalClusterProvider;

/// Type alias for thread-safe mutable LocalClusterProvider.
pub type SharedLocalClusterProvider = ArcShared<ToolboxMutex<LocalClusterProvider<StdToolbox>, StdToolbox>>;

/// Subscribes to remoting lifecycle events for automatic topology updates.
///
/// This function registers a subscriber to the event stream that listens for
/// `RemotingLifecycleEvent::Connected` and `Quarantined` events, automatically
/// triggering `TopologyUpdated` events when nodes join or leave.
///
/// **Note**: This function is only available in std environments with `StdToolbox`.
pub fn subscribe_remoting_events(provider: &SharedLocalClusterProvider) {
  struct RemotingEventHandler {
    provider: SharedLocalClusterProvider,
  }

  impl EventStreamSubscriber<StdToolbox> for RemotingEventHandler {
    fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
      if let EventStreamEvent::Extension { name, payload } = event {
        if name == "remoting" {
          // 起動前は無視
          if !self.provider.lock().is_started() {
            return;
          }
          if let Some(lifecycle_event) = payload.payload().downcast_ref::<RemotingLifecycleEvent>() {
            match lifecycle_event {
              | RemotingLifecycleEvent::Connected { authority, .. } => {
                self.provider.lock().handle_connected(authority);
              },
              | RemotingLifecycleEvent::Quarantined { authority, .. } => {
                self.provider.lock().handle_quarantined(authority);
              },
              | _ => {},
            }
          }
        }
      }
    }
  }

  // event_stream への参照を取得
  let event_stream = provider.lock().event_stream().clone();
  let handler = RemotingEventHandler { provider: provider.clone() };
  let subscriber: EventStreamSubscriberShared<StdToolbox> = subscriber_handle(handler);
  let _subscription: EventStreamSubscriptionGeneric<StdToolbox> =
    EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);
  // Note: subscription は provider のライフタイムに依存するので、
  // provider がドロップされるまで有効
}

/// Creates a shared, thread-safe LocalClusterProvider wrapped in a mutex.
pub fn wrap_local_cluster_provider(provider: LocalClusterProvider<StdToolbox>) -> SharedLocalClusterProvider {
  ArcShared::new(<StdToolbox as fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox>::MutexFamily::create(provider))
}
