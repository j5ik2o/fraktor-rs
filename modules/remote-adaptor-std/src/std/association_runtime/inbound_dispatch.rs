//! Inbound dispatch loop: feeds incoming wire frames into the matching
//! `Association`.

use fraktor_remote_core_rs::core::{
  address::{Address, UniqueAddress},
  extension::EventPublisher,
  transport::TransportError,
  watcher::WatcherCommand,
  wire::{ControlPdu, HandshakePdu, HandshakeReq, HandshakeRsp},
};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::std::{
  association_runtime::{
    apply_effects_in_place, association_registry::AssociationRegistry,
    inbound_quarantine_check::InboundQuarantineCheck, peer_address_match::peer_matches_address,
  },
  tcp_transport::{InboundFrameEvent, WireFrame},
};

/// Reads inbound frames from the TCP transport's inbound channel and
/// dispatches them into the matching `Association`.
pub async fn run_inbound_dispatch(
  mut inbound_rx: UnboundedReceiver<InboundFrameEvent>,
  registry: AssociationRegistry,
  now_ms_provider: impl Fn() -> u64 + Send + 'static,
  event_publisher: EventPublisher,
  sinks: (
    UniqueAddress,
    impl FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static,
    impl FnMut(&Address, ControlPdu) -> Result<(), TransportError> + Send + 'static,
    impl FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>> + Send + 'static,
  ),
) {
  let (local, mut send_handshake_response, mut send_control_response, mut submit_watcher_command) = sinks;
  while let Some(event) = inbound_rx.recv().await {
    if !InboundQuarantineCheck::allows(&registry, &event) {
      tracing::debug!(peer = %event.peer, "dropping inbound frame from quarantined association");
      continue;
    }
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
      | WireFrame::Control(pdu) => {
        let now = now_ms_provider();
        dispatch_control_pdu(
          &event.peer,
          &pdu,
          &registry,
          now,
          &local,
          &mut send_control_response,
          &mut submit_watcher_command,
        );
      },
      | WireFrame::Ack(_pdu) => {
        tracing::debug!(peer = %event.peer, "inbound ack frame received");
      },
    }
  }
}

fn dispatch_control_pdu(
  peer: &str,
  pdu: &ControlPdu,
  registry: &AssociationRegistry,
  now_ms: u64,
  local: &UniqueAddress,
  send_control_response: &mut impl FnMut(&Address, ControlPdu) -> Result<(), TransportError>,
  submit_watcher_command: &mut impl FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>>,
) {
  match pdu {
    | ControlPdu::Heartbeat { authority } => {
      dispatch_heartbeat_request(
        peer,
        authority,
        registry,
        now_ms,
        local,
        send_control_response,
        submit_watcher_command,
      );
    },
    | ControlPdu::HeartbeatResponse { authority, uid } => {
      dispatch_heartbeat_response(peer, authority, *uid, registry, now_ms, submit_watcher_command);
    },
    | ControlPdu::Quarantine { .. } => {
      tracing::debug!(peer = %peer, "inbound quarantine control frame received");
    },
    | ControlPdu::Shutdown { .. } => {
      tracing::debug!(peer = %peer, "inbound shutdown control frame received");
    },
  }
}

fn dispatch_heartbeat_request(
  peer: &str,
  authority: &str,
  registry: &AssociationRegistry,
  now_ms: u64,
  local: &UniqueAddress,
  send_control_response: &mut impl FnMut(&Address, ControlPdu) -> Result<(), TransportError>,
  submit_watcher_command: &mut impl FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>>,
) {
  let Some(remote_address) = registered_remote_address(peer, authority, registry, "heartbeat request") else {
    return;
  };
  // 受信した heartbeat 自身を liveness signal として watcher に流し込み、応答送信に
  // 失敗・遅延したケースでも片方向疎通を検出できるようにする。
  let received_command = WatcherCommand::HeartbeatReceived { from: remote_address.clone(), now: now_ms };
  match submit_watcher_command(received_command) {
    | Ok(()) => {},
    | Err(command) => {
      tracing::warn!(peer = %peer, origin = %remote_address, ?command, "heartbeat received submission failed");
    },
  }
  let response = ControlPdu::HeartbeatResponse { authority: local.address().to_string(), uid: local.uid() };
  match send_control_response(&remote_address, response) {
    | Ok(()) => {},
    | Err(err) => {
      tracing::warn!(peer = %peer, origin = %remote_address, ?err, "heartbeat response send failed");
    },
  }
}

fn dispatch_heartbeat_response(
  peer: &str,
  authority: &str,
  uid: u64,
  registry: &AssociationRegistry,
  now_ms: u64,
  submit_watcher_command: &mut impl FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>>,
) {
  let Some(remote_address) = registered_remote_address(peer, authority, registry, "heartbeat response") else {
    return;
  };
  let command = WatcherCommand::HeartbeatResponseReceived { from: remote_address, uid, now: now_ms };
  match submit_watcher_command(command) {
    | Ok(()) => {},
    | Err(command) => {
      tracing::warn!(peer = %peer, ?command, "watcher command submission failed");
    },
  }
}

fn registered_remote_address(
  peer: &str,
  authority: &str,
  registry: &AssociationRegistry,
  frame_name: &str,
) -> Option<Address> {
  let Some(remote_address) = parse_authority(authority) else {
    tracing::warn!(peer = %peer, authority, frame_name, "discarding control frame with invalid authority");
    return None;
  };
  if !peer_matches_address(peer, &remote_address) {
    tracing::warn!(
      peer = %peer,
      origin = %remote_address,
      frame_name,
      "discarding control frame whose authority does not match the peer socket",
    );
    return None;
  }
  if registry.get_by_remote_address(&remote_address).is_none() {
    tracing::warn!(
      peer = %peer,
      origin = %remote_address,
      frame_name,
      "discarding control frame for an unregistered association",
    );
    return None;
  }
  Some(remote_address)
}

pub(super) fn parse_authority(authority: &str) -> Option<Address> {
  let (system, endpoint) = authority.split_once('@')?;
  // IPv6 リテラルでも最後の `:` がポート区切り。ホスト部のブラケット剥がしは
  // `peer_matches_address` と同じく一律で行う。
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  let port = port.parse::<u16>().ok()?;
  Some(Address::new(system, host, port))
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
  // Lock 区間内で外部 I/O (send_handshake_response) を呼ぶと、send 側が再帰的に同じ
  // registry/association を参照するパスを持っていたとき deadlock し得る。受理判定と
  // effect 適用、応答 PDU の構築までを lock 内で完結させ、送信は lock 解放後に行う。
  let response = target.with_write(|assoc| match assoc.accept_handshake_request(req, now_ms) {
    | Ok(effects) => {
      apply_effects_in_place(assoc, effects, event_publisher);
      Some(HandshakePdu::Rsp(HandshakeRsp::new(local.clone())))
    },
    | Err(err) => {
      tracing::warn!(peer = %peer, ?err, "discarding invalid handshake request");
      None
    },
  });
  match response {
    | Some(response) => match send_handshake_response(remote_address, response) {
      | Ok(()) => {},
      | Err(err) => {
        tracing::warn!(peer = %peer, origin = %remote_address, ?err, "handshake response send failed");
      },
    },
    | None => {},
  }
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
