//! Static cluster provider for static topology scenarios.
//!
//! This provider publishes a static ClusterTopology to EventStream, enabling
//! automatic topology application without network dependencies. Useful for
//! single-process tests and no_std examples.

use alloc::string::String;

use fraktor_actor_rs::core::event_stream::EventStreamGeneric;
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{ClusterProvider, ClusterProviderError, ClusterTopology};

#[cfg(test)]
mod tests;

/// Static cluster provider that publishes static topology to EventStream.
///
/// Unlike network-based providers, this provider does not perform any remote
/// communication. It simply publishes a predetermined topology when started,
/// making it ideal for testing and single-process demonstrations.
pub struct StaticClusterProvider<TB: RuntimeToolbox + 'static> {
  event_stream:        ArcShared<EventStreamGeneric<TB>>,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  static_topology:     Option<ClusterTopology>,
  advertised_address:  String,
}

impl<TB: RuntimeToolbox + 'static> StaticClusterProvider<TB> {
  /// Creates a new static cluster provider.
  #[must_use]
  pub fn new(
    event_stream: ArcShared<EventStreamGeneric<TB>>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    advertised_address: impl Into<String>,
  ) -> Self {
    Self { event_stream, block_list_provider, static_topology: None, advertised_address: advertised_address.into() }
  }

  /// Sets the static topology to be published on startup.
  #[must_use]
  pub fn with_static_topology(mut self, topology: ClusterTopology) -> Self {
    self.static_topology = Some(topology);
    self
  }

  /// Returns the advertised address.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn advertised_address(&self) -> &str {
    &self.advertised_address
  }

  /// Publishes the static topology to EventStream.
  fn publish_topology(&self) {
    use fraktor_actor_rs::core::{event_stream::EventStreamEvent, messaging::AnyMessageGeneric};

    use crate::core::ClusterEvent;

    if let Some(topology) = &self.static_topology {
      let blocked = self.block_list_provider.blocked_members();
      let event = ClusterEvent::TopologyUpdated {
        topology: topology.clone(),
        joined: topology.joined().clone(),
        left: topology.left().clone(),
        blocked,
      };
      let payload = AnyMessageGeneric::new(event);
      let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
      self.event_stream.publish(&extension_event);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ClusterProvider for StaticClusterProvider<TB> {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    self.publish_topology();
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    self.publish_topology();
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    // 静的 provider なので特にクリーンアップ不要
    Ok(())
  }
}
