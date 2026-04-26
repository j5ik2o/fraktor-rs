//! Inbound dispatch loop: feeds incoming wire frames into the matching
//! `Association`.

use fraktor_remote_core_rs::core::{
  address::{Address, UniqueAddress},
  extension::EventPublisher,
  transport::TransportError,
  wire::{HandshakePdu, HandshakeReq, HandshakeRsp},
};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::std::{
  association_runtime::{apply_effects_in_place, association_registry::AssociationRegistry},
  tcp_transport::{InboundFrameEvent, WireFrame},
};

/// Reads inbound frames from the TCP transport's inbound channel and
/// dispatches them into the matching `Association`.
pub async fn run_inbound_dispatch(
  mut inbound_rx: UnboundedReceiver<InboundFrameEvent>,
  registry: AssociationRegistry,
  now_ms_provider: impl Fn() -> u64 + Send + 'static,
  event_publisher: EventPublisher,
  local: UniqueAddress,
  mut send_handshake_response: impl FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static,
) {
  while let Some(event) = inbound_rx.recv().await {
    match event.frame {
      | WireFrame::Handshake(pdu) => {
        let now = now_ms_provider();
        dispatch_handshake_pdu(
          &event.peer,
          &pdu,
          &registry,
          now,
          &event_publisher,
          &local,
          &mut send_handshake_response,
        );
      },
      | WireFrame::Envelope(_pdu) => {
        // Local actor delivery is a separate provider integration contract.
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

fn dispatch_handshake_pdu(
  peer: &str,
  pdu: &HandshakePdu,
  registry: &AssociationRegistry,
  now_ms: u64,
  event_publisher: &EventPublisher,
  local: &UniqueAddress,
  send_handshake_response: &mut impl FnMut(&Address, HandshakePdu) -> Result<(), TransportError>,
) {
  match pdu {
    | HandshakePdu::Req(req) => {
      dispatch_handshake_request(peer, req, registry, now_ms, event_publisher, local, send_handshake_response);
    },
    | HandshakePdu::Rsp(rsp) => dispatch_handshake_response(peer, rsp, registry, now_ms, event_publisher),
  }
}

fn dispatch_handshake_request(
  peer: &str,
  req: &HandshakeReq,
  registry: &AssociationRegistry,
  now_ms: u64,
  event_publisher: &EventPublisher,
  local: &UniqueAddress,
  send_handshake_response: &mut impl FnMut(&Address, HandshakePdu) -> Result<(), TransportError>,
) {
  let remote_address = req.from().address();
  let Some(target) = registry.get_by_remote_address(remote_address).cloned() else {
    tracing::warn!(
      peer = %peer,
      origin = %remote_address,
      "discarding handshake request for an unregistered association",
    );
    return;
  };
  target.with_write(|assoc| match assoc.accept_handshake_request(req, now_ms) {
    | Ok(effects) => {
      apply_effects_in_place(assoc, effects, event_publisher);
      let response = HandshakePdu::Rsp(HandshakeRsp::new(local.clone()));
      if let Err(err) = send_handshake_response(remote_address, response) {
        tracing::warn!(peer = %peer, origin = %remote_address, ?err, "handshake response send failed");
      }
    },
    | Err(err) => {
      tracing::warn!(peer = %peer, ?err, "discarding invalid handshake request");
    },
  });
}

fn dispatch_handshake_response(
  peer: &str,
  rsp: &HandshakeRsp,
  registry: &AssociationRegistry,
  now_ms: u64,
  event_publisher: &EventPublisher,
) {
  let remote_address = rsp.from().address();
  let Some(target) = registry.get_by_remote_address(remote_address).cloned() else {
    tracing::warn!(
      peer = %peer,
      origin = %remote_address,
      "discarding handshake response for an unregistered association",
    );
    return;
  };
  target.with_write(|assoc| match assoc.accept_handshake_response(rsp, now_ms) {
    | Ok(effects) => apply_effects_in_place(assoc, effects, event_publisher),
    | Err(err) => {
      tracing::warn!(peer = %peer, ?err, "discarding invalid handshake response");
    },
  });
}
