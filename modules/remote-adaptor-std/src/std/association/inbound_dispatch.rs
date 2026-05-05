//! Inbound I/O worker that forwards decoded TCP frames to `Remote`.

#[cfg(test)]
mod tests;

use core::time::Duration;

use bytes::BytesMut;
use fraktor_remote_core_rs::core::{
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

/// Reads decoded inbound frames and pushes raw core `RemoteEvent`s.
///
/// # Errors
///
/// Returns [`TransportError::NotAvailable`] when the remote event receiver has
/// already closed.
pub async fn run_inbound_dispatch(
  inbound_rx: UnboundedReceiver<InboundFrameEvent>,
  event_sender: Sender<RemoteEvent>,
  now_ms_provider: impl Fn() -> u64 + Send + 'static,
  frame_codec: WireFrameCodec,
  inbound_max_restarts: u32,
  inbound_restart_timeout: Duration,
) -> Result<(), TransportError> {
  let mut inbound_rx = inbound_rx;
  let started_at = Instant::now();
  let mut restart_counter = RestartCounter::new(inbound_max_restarts, inbound_restart_timeout);
  loop {
    match run_inbound_dispatch_once(&mut inbound_rx, event_sender.clone(), &now_ms_provider, frame_codec).await {
      | Ok(()) => return Ok(()),
      | Err(error) => {
        if !restart_counter.restart(tokio_instant_elapsed_millis(started_at)) {
          return Err(error);
        }
      },
    }
  }
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
    if let Err(error) = frame_codec.encode(event.frame, &mut bytes) {
      tracing::warn!(?error, "inbound frame re-encoding failed; skipping frame");
      continue;
    }
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
