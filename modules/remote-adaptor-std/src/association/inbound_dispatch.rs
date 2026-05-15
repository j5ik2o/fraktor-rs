//! Inbound I/O worker that forwards decoded TCP frames to `Remote`.

#[cfg(test)]
#[path = "inbound_dispatch_test.rs"]
mod tests;

use fraktor_remote_core_rs::{
  extension::RemoteEvent,
  transport::{TransportEndpoint, TransportError},
  wire::HandshakePdu,
};
use tokio::sync::mpsc::{Sender, UnboundedReceiver};

use crate::transport::tcp::{InboundFrameEvent, WireFrame};

/// Reads decoded inbound frames and pushes raw core `RemoteEvent`s.
///
/// # Errors
///
/// Returns [`TransportError::NotAvailable`] when the remote event receiver has
/// already closed.
pub async fn run_inbound_dispatch(
  mut inbound_rx: UnboundedReceiver<InboundFrameEvent>,
  event_sender: Sender<RemoteEvent>,
  now_ms_provider: impl Fn() -> u64 + Send + 'static,
) -> Result<(), TransportError> {
  while let Some(event) = inbound_rx.recv().await {
    let authority = event
      .authority
      .or_else(|| authority_for_frame(&event.frame))
      .unwrap_or_else(|| TransportEndpoint::new(event.peer.clone()));
    let remote_event = RemoteEvent::InboundFrameReceived { authority, frame: event.frame, now_ms: now_ms_provider() };
    if let Err(error) = event_sender.send(remote_event).await {
      tracing::warn!(?error, "inbound remote event delivery failed");
      return Err(TransportError::NotAvailable);
    }
  }
  Ok(())
}

pub(crate) fn authority_for_frame(frame: &WireFrame) -> Option<TransportEndpoint> {
  match frame {
    | WireFrame::Handshake(HandshakePdu::Req(request)) => {
      Some(TransportEndpoint::new(request.from().address().to_string()))
    },
    | WireFrame::Handshake(HandshakePdu::Rsp(response)) => {
      Some(TransportEndpoint::new(response.from().address().to_string()))
    },
    | WireFrame::Control(_) | WireFrame::Envelope(_) | WireFrame::Ack(_) | WireFrame::Deployment(_) => None,
  }
}
