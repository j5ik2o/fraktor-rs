//! Inbound quarantine filter for association runtime frames.

use crate::std::{
  association::{association_registry::AssociationRegistry, peer_address_match::peer_matches_address},
  tcp_transport::InboundFrameEvent,
};

/// Decides whether an inbound frame may pass a quarantined association.
pub struct InboundQuarantineCheck;

impl InboundQuarantineCheck {
  /// Returns `true` when `event` may continue through inbound dispatch.
  #[must_use]
  pub fn allows(registry: &AssociationRegistry, event: &InboundFrameEvent) -> bool {
    let Some(is_quarantined) = association_is_quarantined(registry, &event.peer) else {
      return true;
    };
    !is_quarantined
  }
}

fn association_is_quarantined(registry: &AssociationRegistry, peer: &str) -> Option<bool> {
  registry
    .iter()
    .find_map(|(remote, shared)| peer_matches_address(peer, remote.address()).then_some(shared))
    .map(|shared| shared.with_write(|association| association.state().is_quarantined()))
}
