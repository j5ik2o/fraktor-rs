//! Inbound I/O worker that forwards decoded TCP frames to `Remote`.

#[cfg(test)]
mod tests;

use core::future::Future;

use bytes::BytesMut;
use fraktor_remote_core_rs::core::{
  config::RemoteConfig,
  extension::RemoteEvent,
  transport::{TransportEndpoint, TransportError},
  wire::{ControlPdu, HandshakePdu},
};
use tokio::{
  sync::mpsc::{Sender, UnboundedReceiver},
  time::Instant,
};
use tokio_util::codec::Encoder;

use crate::std::{
  association::{RestartCounter, tokio_instant_elapsed_millis},
  transport::tcp::{InboundFrameEvent, WireFrame, WireFrameCodec},
};

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
        if !restart_counter.restart(tokio_instant_elapsed_millis(started_at)) {
          return Err(err);
        }
      },
    }
  }
}

/// Reads decoded inbound frames and pushes raw core `RemoteEvent`s.
///
/// # Errors
///
/// Returns [`TransportError::NotAvailable`] when frame encoding fails or the
/// remote event receiver has already closed.
pub async fn run_inbound_dispatch(
  inbound_rx: UnboundedReceiver<InboundFrameEvent>,
  event_sender: Sender<RemoteEvent>,
  now_ms_provider: impl Fn() -> u64 + Send + 'static,
  frame_codec: WireFrameCodec,
) -> Result<(), TransportError> {
  let mut inbound_rx = inbound_rx;
  run_inbound_dispatch_once(&mut inbound_rx, event_sender, &now_ms_provider, frame_codec).await
}

async fn run_inbound_dispatch_once(
  inbound_rx: &mut UnboundedReceiver<InboundFrameEvent>,
  event_sender: Sender<RemoteEvent>,
  now_ms_provider: &impl Fn() -> u64,
  mut frame_codec: WireFrameCodec,
) -> Result<(), TransportError> {
  while let Some(event) = inbound_rx.recv().await {
    let authority = authority_for_frame(&event.frame).unwrap_or_else(|| TransportEndpoint::new(event.peer.clone()));
    let mut bytes = BytesMut::new();
    frame_codec.encode(event.frame, &mut bytes).map_err(|error| {
      tracing::warn!(?error, "inbound frame re-encoding failed");
      TransportError::NotAvailable
    })?;
    let remote_event =
      RemoteEvent::InboundFrameReceived { authority, frame: bytes.freeze().to_vec(), now_ms: now_ms_provider() };
    if let Err(error) = event_sender.send(remote_event).await {
      tracing::warn!(?error, "inbound remote event delivery failed");
      return Err(TransportError::NotAvailable);
    }
  }
  Ok(())
}

fn authority_for_frame(frame: &WireFrame) -> Option<TransportEndpoint> {
  match frame {
    | WireFrame::Handshake(HandshakePdu::Req(request)) => {
      Some(TransportEndpoint::new(request.from().address().to_string()))
    },
    | WireFrame::Handshake(HandshakePdu::Rsp(response)) => {
      Some(TransportEndpoint::new(response.from().address().to_string()))
    },
    | WireFrame::Control(ControlPdu::Heartbeat { authority })
    | WireFrame::Control(ControlPdu::HeartbeatResponse { authority, .. })
    | WireFrame::Control(ControlPdu::Quarantine { authority, .. })
    | WireFrame::Control(ControlPdu::Shutdown { authority }) => Some(TransportEndpoint::new(authority.clone())),
    | WireFrame::Envelope(pdu) => pdu.sender_path().and_then(authority_from_actor_path).map(TransportEndpoint::new),
    | WireFrame::Ack(_) => None,
  }
}

fn authority_from_actor_path(path: &str) -> Option<String> {
  let (_scheme, rest) = path.split_once("://")?;
  let (authority, _path) = rest.split_once('/')?;
  Some(authority.to_owned())
}
