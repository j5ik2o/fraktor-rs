//! Inbound dispatch loop: feeds incoming wire frames into the matching
//! `Association`.

use fraktor_remote_core_rs::address::RemoteNodeId;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
  association_runtime::{apply_effects_in_place, association_shared::AssociationShared},
  tcp_transport::{InboundFrameEvent, WireFrame},
};

/// Reads inbound frames from the TCP transport's inbound channel and
/// dispatches them into the matching `Association`.
///
/// This Phase B minimum-viable implementation maps every inbound frame to
/// the single `target` `AssociationShared` passed in. Section 21's provider
/// dispatch and Section 22's `StdRemoting` will replace this with a per-peer
/// lookup once the full peer registry is wired up.
///
/// The function returns when:
///
/// - `inbound_rx` is closed by the transport, or
/// - `now_ms_provider` returns a value indicating the runtime is shutting down (currently never).
pub async fn run_inbound_dispatch(
  mut inbound_rx: UnboundedReceiver<InboundFrameEvent>,
  target: AssociationShared,
  now_ms_provider: impl Fn() -> u64 + Send + 'static,
) {
  while let Some(event) = inbound_rx.recv().await {
    match event.frame {
      | WireFrame::Handshake(_pdu) => {
        // Phase B minimum: synthesise a remote-node identifier from the
        // peer string. The full handshake protocol (validating origin
        // system / uid against the PDU contents) is left to Section 22's
        // StdRemoting wiring.
        let now = now_ms_provider();
        let remote_node = RemoteNodeId::new("remote", event.peer.as_str(), None, 0);
        target.with_write(|assoc| {
          let effects = assoc.handshake_accepted(remote_node, now);
          // The state is now Active. apply_effects_in_place re-enqueues any
          // deferred envelopes through `assoc.enqueue` so the outbound loop
          // drains them, and logs the lifecycle event. Discarding `effects`
          // here would silently lose every message buffered during the
          // handshake.
          apply_effects_in_place(assoc, effects);
        });
      },
      | WireFrame::Envelope(_pdu) => {
        // Phase B minimum: the envelope is observed but not delivered to a
        // local actor — that wiring belongs in Section 22 once the
        // `StdRemoteActorRefProvider` is in place.
        tracing::debug!(peer = %event.peer, "inbound envelope frame received");
      },
      | WireFrame::Control(_pdu) => {
        tracing::debug!(peer = %event.peer, "inbound control frame received");
      },
      | WireFrame::Ack(_pdu) => {
        tracing::debug!(peer = %event.peer, "inbound ack frame received");
      },
    }
  }
}
