//! Inbound dispatch loop: feeds incoming wire frames into the matching
//! `Association`.

use fraktor_remote_core_rs::core::{address::RemoteNodeId, extension::EventPublisher, wire::HandshakePdu};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::std::{
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
  event_publisher: EventPublisher,
) {
  while let Some(event) = inbound_rx.recv().await {
    match event.frame {
      | WireFrame::Handshake(pdu) => {
        let now = now_ms_provider();
        let remote_node = remote_node_from_handshake_pdu(&pdu);
        target.with_write(|assoc| {
          let effects = assoc.handshake_accepted(remote_node, now);
          // The state is now Active. apply_effects_in_place re-enqueues any
          // deferred envelopes through `assoc.enqueue` so the outbound loop
          // drains them and publishes the lifecycle event. Discarding
          // `effects` here would silently lose every message buffered during
          // the handshake.
          apply_effects_in_place(assoc, effects, &event_publisher);
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

fn remote_node_from_handshake_pdu(pdu: &HandshakePdu) -> RemoteNodeId {
  match pdu {
    | HandshakePdu::Req(req) => {
      RemoteNodeId::new(req.origin_system(), req.origin_host(), Some(req.origin_port()), req.origin_uid())
    },
    | HandshakePdu::Rsp(rsp) => {
      RemoteNodeId::new(rsp.origin_system(), rsp.origin_host(), Some(rsp.origin_port()), rsp.origin_uid())
    },
  }
}
