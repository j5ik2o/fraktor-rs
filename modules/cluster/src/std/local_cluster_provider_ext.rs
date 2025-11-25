//! std-only extension for LocalClusterProvider.
//!
//! This module provides Transport event auto-detection functionality
//! that is only available in std environments.

use fraktor_actor_rs::core::event_stream::{
  EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric, RemotingLifecycleEvent,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::core::LocalClusterProvider;

/// Subscribes to remoting lifecycle events for automatic topology updates.
///
/// This function registers a subscriber to the event stream that listens for
/// `RemotingLifecycleEvent::Connected` and `Quarantined` events, automatically
/// triggering `TopologyUpdated` events when nodes join or leave.
///
/// **Note**: This function is only available in std environments with `StdToolbox`.
pub fn subscribe_remoting_events(provider: &ArcShared<LocalClusterProvider<StdToolbox>>) {
  struct RemotingEventHandler {
    provider: ArcShared<LocalClusterProvider<StdToolbox>>,
  }

  impl EventStreamSubscriber<StdToolbox> for RemotingEventHandler {
    fn on_event(&self, event: &EventStreamEvent<StdToolbox>) {
      if let EventStreamEvent::Extension { name, payload } = event {
        if name == "remoting" {
          // 起動前は無視
          if !self.provider.is_started() {
            return;
          }
          if let Some(lifecycle_event) = payload.payload().downcast_ref::<RemotingLifecycleEvent>() {
            match lifecycle_event {
              | RemotingLifecycleEvent::Connected { authority, .. } => {
                self.provider.handle_connected(authority);
              },
              | RemotingLifecycleEvent::Quarantined { authority, .. } => {
                self.provider.handle_quarantined(authority);
              },
              | _ => {},
            }
          }
        }
      }
    }
  }

  let handler = RemotingEventHandler { provider: provider.clone() };
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = ArcShared::new(handler);
  let _subscription: EventStreamSubscriptionGeneric<StdToolbox> =
    EventStreamGeneric::subscribe_arc(provider.event_stream(), &subscriber);
  // Note: subscription は provider のライフタイムに依存するので、
  // provider がドロップされるまで有効
}
