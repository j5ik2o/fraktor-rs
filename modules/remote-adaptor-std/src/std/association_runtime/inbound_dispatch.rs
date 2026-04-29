//! Inbound dispatch loop: feeds incoming wire frames into the matching
//! `Association`.

use core::{future::Future, time::Duration};
use std::sync::{Arc, Mutex};

use fraktor_remote_core_rs::core::{
  address::{Address, UniqueAddress},
  config::RemoteConfig,
  extension::EventPublisher,
  transport::TransportError,
  watcher::WatcherCommand,
  wire::{ControlPdu, HandshakePdu, HandshakeReq, HandshakeRsp},
};
use tokio::{sync::mpsc::UnboundedReceiver, time::Instant};

use crate::std::{
  association_runtime::{
    RestartCounter, apply_effects_in_place, association_registry::AssociationRegistry,
    inbound_quarantine_check::InboundQuarantineCheck, peer_address_match::peer_matches_address,
  },
  tcp_transport::{InboundFrameEvent, WireFrame},
};

struct InboundDispatchState<HS, CS, WS> {
  inbound_rx:              UnboundedReceiver<InboundFrameEvent>,
  send_handshake_response: HS,
  send_control_response:   CS,
  submit_watcher_command:  WS,
}

struct InboundDispatchContext<'a, Now, HS, CS, WS> {
  registry:                &'a AssociationRegistry,
  now_ms_provider:         &'a Now,
  event_publisher:         &'a EventPublisher,
  local:                   &'a UniqueAddress,
  send_handshake_response: &'a mut HS,
  send_control_response:   &'a mut CS,
  submit_watcher_command:  &'a mut WS,
}

/// Re-runs an inbound task until it succeeds or the configured restart budget
/// is exhausted.
///
/// # Errors
///
/// Returns the last task error once the configured inbound restart budget is
/// consumed inside the active restart-timeout window.
pub async fn run_inbound_task_with_restart_budget<F, Fut, E>(config: &RemoteConfig, mut run_task: F) -> Result<(), E>
where
  F: FnMut() -> Fut,
  Fut: Future<Output = Result<(), E>>, {
  let started_at = Instant::now();
  let mut restart_counter = RestartCounter::new(config.inbound_max_restarts(), config.inbound_restart_timeout());

  loop {
    match run_task().await {
      | Ok(()) => return Ok(()),
      | Err(err) => {
        if !restart_counter.restart(elapsed_ms(started_at)) {
          return Err(err);
        }
      },
    }
  }
}

/// Reads inbound frames from the TCP transport's inbound channel and
/// dispatches them into the matching `Association`.
///
/// # Errors
///
/// Returns the last transport send error when handshake/control response
/// delivery keeps failing until the configured inbound restart budget is
/// exhausted.
pub async fn run_inbound_dispatch<HS, CS, WS>(
  config: &RemoteConfig,
  inbound_rx: UnboundedReceiver<InboundFrameEvent>,
  registry: AssociationRegistry,
  now_ms_provider: impl Fn() -> u64 + Send + 'static,
  event_publisher: EventPublisher,
  sinks: (UniqueAddress, HS, CS, WS),
) -> Result<(), TransportError>
where
  HS: FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static,
  CS: FnMut(&Address, ControlPdu) -> Result<(), TransportError> + Send + 'static,
  WS: FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>> + Send + 'static, {
  let (local, send_handshake_response, send_control_response, submit_watcher_command) = sinks;
  let state = Arc::new(Mutex::new(Some(InboundDispatchState {
    inbound_rx,
    send_handshake_response,
    send_control_response,
    submit_watcher_command,
  })));
  let registry = Arc::new(registry);
  let now_ms_provider = Arc::new(now_ms_provider);
  run_inbound_task_with_restart_budget(config, {
    let state = Arc::clone(&state);
    let registry = Arc::clone(&registry);
    let now_ms_provider = Arc::clone(&now_ms_provider);
    let event_publisher = event_publisher.clone();
    let local = local.clone();
    move || {
      let state = Arc::clone(&state);
      let registry = Arc::clone(&registry);
      let now_ms_provider = Arc::clone(&now_ms_provider);
      let event_publisher = event_publisher.clone();
      let local = local.clone();
      async move {
        let mut dispatch_state = {
          let mut guard = match state.lock() {
            | Ok(guard) => guard,
            | Err(poisoned) => poisoned.into_inner(),
          };
          match guard.take() {
            | Some(state) => state,
            | None => unreachable!("inbound dispatch state must be available before restart attempt"),
          }
        };
        let ctx = InboundDispatchContext {
          registry:                registry.as_ref(),
          now_ms_provider:         now_ms_provider.as_ref(),
          event_publisher:         &event_publisher,
          local:                   &local,
          send_handshake_response: &mut dispatch_state.send_handshake_response,
          send_control_response:   &mut dispatch_state.send_control_response,
          submit_watcher_command:  &mut dispatch_state.submit_watcher_command,
        };
        let result = run_inbound_dispatch_once(&mut dispatch_state.inbound_rx, ctx).await;
        let mut guard = match state.lock() {
          | Ok(guard) => guard,
          | Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(dispatch_state);
        result
      }
    }
  })
  .await
}

async fn run_inbound_dispatch_once<Now, HS, CS, WS>(
  inbound_rx: &mut UnboundedReceiver<InboundFrameEvent>,
  ctx: InboundDispatchContext<'_, Now, HS, CS, WS>,
) -> Result<(), TransportError>
where
  Now: Fn() -> u64,
  HS: FnMut(&Address, HandshakePdu) -> Result<(), TransportError>,
  CS: FnMut(&Address, ControlPdu) -> Result<(), TransportError>,
  WS: FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>>, {
  let InboundDispatchContext {
    registry,
    now_ms_provider,
    event_publisher,
    local,
    send_handshake_response,
    send_control_response,
    submit_watcher_command,
  } = ctx;
  while let Some(event) = inbound_rx.recv().await {
    if !InboundQuarantineCheck::allows(registry, &event) {
      tracing::debug!(peer = %event.peer, "dropping inbound frame from quarantined association");
      continue;
    }
    match event.frame {
      | WireFrame::Handshake(pdu) => {
        let now = now_ms_provider();
        dispatch_handshake_pdu(&event.peer, &pdu, registry, now, event_publisher, local, send_handshake_response)?;
      },
      | WireFrame::Envelope(_pdu) => {
        // Local actor delivery is a separate provider integration contract.
        tracing::debug!(peer = %event.peer, "inbound envelope frame received");
      },
      | WireFrame::Control(pdu) => {
        let now = now_ms_provider();
        dispatch_control_pdu(&event.peer, &pdu, registry, now, local, send_control_response, submit_watcher_command)?;
      },
      | WireFrame::Ack(_pdu) => {
        tracing::debug!(peer = %event.peer, "inbound ack frame received");
      },
    }
  }
  Ok(())
}

fn dispatch_control_pdu(
  peer: &str,
  pdu: &ControlPdu,
  registry: &AssociationRegistry,
  now_ms: u64,
  local: &UniqueAddress,
  send_control_response: &mut impl FnMut(&Address, ControlPdu) -> Result<(), TransportError>,
  submit_watcher_command: &mut impl FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>>,
) -> Result<(), TransportError> {
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
      )?;
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
  Ok(())
}

fn dispatch_heartbeat_request(
  peer: &str,
  authority: &str,
  registry: &AssociationRegistry,
  now_ms: u64,
  local: &UniqueAddress,
  send_control_response: &mut impl FnMut(&Address, ControlPdu) -> Result<(), TransportError>,
  submit_watcher_command: &mut impl FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>>,
) -> Result<(), TransportError> {
  let Some(remote_address) = registered_remote_address(peer, authority, registry, "heartbeat request") else {
    return Ok(());
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
    | Ok(()) => Ok(()),
    | Err(err) => {
      tracing::warn!(peer = %peer, origin = %remote_address, ?err, "heartbeat response send failed");
      Err(err)
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
  let origin = remote_address.clone();
  let command = WatcherCommand::HeartbeatResponseReceived { from: remote_address, uid, now: now_ms };
  match submit_watcher_command(command) {
    | Ok(()) => {},
    | Err(command) => {
      tracing::warn!(peer = %peer, origin = %origin, ?command, "watcher command submission failed");
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
) -> Result<(), TransportError> {
  match pdu {
    | HandshakePdu::Req(req) => {
      dispatch_handshake_request(peer, req, registry, now_ms, event_publisher, local, send_handshake_response)?;
    },
    | HandshakePdu::Rsp(rsp) => dispatch_handshake_response(peer, rsp, registry, now_ms, event_publisher),
  }
  Ok(())
}

fn dispatch_handshake_request(
  peer: &str,
  req: &HandshakeReq,
  registry: &AssociationRegistry,
  now_ms: u64,
  event_publisher: &EventPublisher,
  local: &UniqueAddress,
  send_handshake_response: &mut impl FnMut(&Address, HandshakePdu) -> Result<(), TransportError>,
) -> Result<(), TransportError> {
  let remote_address = req.from().address();
  let Some(target) = registry.get_by_remote_address(remote_address).cloned() else {
    tracing::warn!(
      peer = %peer,
      origin = %remote_address,
      "discarding handshake request for an unregistered association",
    );
    return Ok(());
  };
  // Lock 区間内で外部 I/O (send_handshake_response) を呼ぶと、send 側が再帰的に同じ
  // registry/association を参照するパスを持っていたとき deadlock し得る。受理判定と
  // effect 適用、応答 PDU の構築までを lock 内で完結させ、送信は lock 解放後に行う。
  let response = target.with_write(|assoc| match assoc.accept_handshake_request(req, now_ms) {
    | Ok(effects) => {
      apply_effects_in_place(assoc, effects, event_publisher, now_ms);
      Some(HandshakePdu::Rsp(HandshakeRsp::new(local.clone())))
    },
    | Err(err) => {
      tracing::warn!(peer = %peer, ?err, "discarding invalid handshake request");
      None
    },
  });
  match response {
    | Some(response) => match send_handshake_response(remote_address, response) {
      | Ok(()) => Ok(()),
      | Err(err) => {
        tracing::warn!(peer = %peer, origin = %remote_address, ?err, "handshake response send failed");
        Err(err)
      },
    },
    | None => Ok(()),
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
    | Ok(effects) => apply_effects_in_place(assoc, effects, event_publisher, now_ms),
    | Err(err) => {
      tracing::warn!(peer = %peer, ?err, "discarding invalid handshake response");
    },
  });
}

fn elapsed_ms(started_at: Instant) -> u64 {
  duration_millis(started_at.elapsed())
}

fn duration_millis(duration: Duration) -> u64 {
  duration.as_millis().min(u128::from(u64::MAX)) as u64
}
