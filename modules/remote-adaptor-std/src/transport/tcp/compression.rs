//! TCP transport-local compression table runtime.

#[cfg(test)]
#[path = "compression_test.rs"]
mod tests;

use alloc::string::{String, ToString};
use core::{future::pending, num::NonZeroUsize, time::Duration};

use fraktor_remote_core_rs::{
  config::RemoteCompressionConfig,
  transport::TransportEndpoint,
  wire::{CompressedText, CompressionTable, CompressionTableKind, ControlPdu, EnvelopePayload, EnvelopePdu, WireError},
};
use tokio::time::{Instant as TokioInstant, Interval, interval_at};

use super::WireFrame;

#[derive(Debug)]
pub(crate) enum InboundCompressionAction {
  Forward(WireFrame),
  Reply { pdu: ControlPdu, authority: TransportEndpoint },
  Consumed { authority: TransportEndpoint },
}

pub(crate) struct TcpCompressionTables {
  outbound_actor_refs: CompressionTable,
  outbound_manifests:  CompressionTable,
  inbound_actor_refs:  CompressionTable,
  inbound_manifests:   CompressionTable,
}

impl TcpCompressionTables {
  pub(crate) fn new(config: RemoteCompressionConfig) -> Self {
    Self {
      outbound_actor_refs: CompressionTable::new(config.actor_ref_max()),
      outbound_manifests:  CompressionTable::new(config.manifest_max()),
      inbound_actor_refs:  CompressionTable::new(config.actor_ref_max()),
      inbound_manifests:   CompressionTable::new(config.manifest_max()),
    }
  }

  pub(crate) fn apply_outbound_frame(&mut self, frame: WireFrame) -> WireFrame {
    match frame {
      | WireFrame::Envelope(pdu) => WireFrame::Envelope(self.apply_outbound_envelope(pdu)),
      | frame => frame,
    }
  }

  pub(crate) fn handle_inbound_frame(
    &mut self,
    frame: WireFrame,
    local_authority: &str,
  ) -> Result<InboundCompressionAction, WireError> {
    match frame {
      | WireFrame::Envelope(pdu) => {
        self.resolve_inbound_envelope(pdu).map(WireFrame::Envelope).map(InboundCompressionAction::Forward)
      },
      | WireFrame::Control(ControlPdu::CompressionAdvertisement { authority, table_kind, generation, entries }) => {
        let authority = TransportEndpoint::new(authority);
        self.inbound_table_mut(table_kind).apply_advertisement(generation, &entries)?;
        Ok(InboundCompressionAction::Reply {
          pdu: ControlPdu::CompressionAck { authority: local_authority.to_string(), table_kind, generation },
          authority,
        })
      },
      | WireFrame::Control(ControlPdu::CompressionAck { authority, table_kind, generation }) => {
        let authority = TransportEndpoint::new(authority);
        self.outbound_table_mut(table_kind).acknowledge(generation);
        Ok(InboundCompressionAction::Consumed { authority })
      },
      | frame => Ok(InboundCompressionAction::Forward(frame)),
    }
  }

  pub(crate) fn create_advertisement(
    &mut self,
    table_kind: CompressionTableKind,
    local_authority: &str,
  ) -> Option<WireFrame> {
    self.outbound_table_mut(table_kind).create_advertisement(table_kind).map(|advertisement| {
      WireFrame::Control(ControlPdu::CompressionAdvertisement {
        authority:  local_authority.to_string(),
        table_kind: advertisement.table_kind(),
        generation: advertisement.generation(),
        entries:    advertisement.into_entries(),
      })
    })
  }

  fn apply_outbound_envelope(&mut self, pdu: EnvelopePdu) -> EnvelopePdu {
    let recipient_path = observe_and_encode(&mut self.outbound_actor_refs, pdu.recipient_path_metadata());
    let sender_path =
      pdu.sender_path_metadata().map(|metadata| observe_and_encode(&mut self.outbound_actor_refs, metadata));
    let manifest = pdu.manifest_metadata().map(|metadata| observe_and_encode(&mut self.outbound_manifests, metadata));
    EnvelopePdu::new_with_metadata(
      recipient_path,
      sender_path,
      pdu.correlation_hi(),
      pdu.correlation_lo(),
      pdu.priority(),
      EnvelopePayload::new(pdu.serializer_id(), None, pdu.payload().clone()),
      manifest,
    )
    .with_redelivery_sequence(pdu.redelivery_sequence())
  }

  fn resolve_inbound_envelope(&self, pdu: EnvelopePdu) -> Result<EnvelopePdu, WireError> {
    let recipient_path =
      CompressedText::literal(resolve_text(&self.inbound_actor_refs, pdu.recipient_path_metadata())?);
    let sender_path =
      pdu.sender_path_metadata().map(|metadata| resolve_text(&self.inbound_actor_refs, metadata)).transpose()?;
    let manifest =
      pdu.manifest_metadata().map(|metadata| resolve_text(&self.inbound_manifests, metadata)).transpose()?;
    Ok(
      EnvelopePdu::new_with_metadata(
        recipient_path,
        sender_path.map(CompressedText::literal),
        pdu.correlation_hi(),
        pdu.correlation_lo(),
        pdu.priority(),
        EnvelopePayload::new(pdu.serializer_id(), None, pdu.payload().clone()),
        manifest.map(CompressedText::literal),
      )
      .with_redelivery_sequence(pdu.redelivery_sequence()),
    )
  }

  fn inbound_table_mut(&mut self, table_kind: CompressionTableKind) -> &mut CompressionTable {
    match table_kind {
      | CompressionTableKind::ActorRef => &mut self.inbound_actor_refs,
      | CompressionTableKind::Manifest => &mut self.inbound_manifests,
    }
  }

  fn outbound_table_mut(&mut self, table_kind: CompressionTableKind) -> &mut CompressionTable {
    match table_kind {
      | CompressionTableKind::ActorRef => &mut self.outbound_actor_refs,
      | CompressionTableKind::Manifest => &mut self.outbound_manifests,
    }
  }
}

pub(crate) fn compression_advertisement_interval(max: Option<NonZeroUsize>, duration: Duration) -> Option<Interval> {
  max?;
  Some(interval_at(TokioInstant::now() + duration, duration))
}

pub(crate) async fn next_compression_advertisement_tick(interval: &mut Option<Interval>) {
  match interval {
    | Some(interval) => {
      interval.tick().await;
    },
    | None => pending::<()>().await,
  }
}

fn observe_and_encode(table: &mut CompressionTable, metadata: &CompressedText) -> CompressedText {
  let Some(literal) = metadata.as_literal() else {
    return metadata.clone();
  };
  table.observe(literal);
  table.encode(literal)
}

fn resolve_text(table: &CompressionTable, metadata: &CompressedText) -> Result<String, WireError> {
  match metadata {
    | CompressedText::Literal(literal) => Ok(literal.clone()),
    | CompressedText::TableRef(entry_id) => {
      table.resolve(*entry_id).map(ToString::to_string).ok_or(WireError::InvalidFormat)
    },
  }
}
