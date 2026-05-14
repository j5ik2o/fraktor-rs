use alloc::{string::ToString, vec};
use core::{num::NonZeroUsize, time::Duration};

use bytes::Bytes;
use fraktor_remote_core_rs::{
  config::RemoteCompressionConfig,
  wire::{
    CompressedText, CompressionTable, CompressionTableEntry, CompressionTableKind, ControlPdu, EnvelopePayload,
    EnvelopePdu, WireError,
  },
};

use super::{
  InboundCompressionAction, TcpCompressionTables, compression_advertisement_interval,
  next_compression_advertisement_tick, observe_and_encode,
};
use crate::transport::tcp::WireFrame;

fn max(value: usize) -> Option<NonZeroUsize> {
  NonZeroUsize::new(value)
}

fn envelope_frame(
  recipient_path: CompressedText,
  sender_path: Option<CompressedText>,
  manifest: Option<CompressedText>,
) -> WireFrame {
  WireFrame::Envelope(
    EnvelopePdu::new_with_metadata(recipient_path, sender_path, (1, 2), 1, 7, manifest, Bytes::from_static(b"hello"))
      .with_redelivery_sequence(None),
  )
}

fn literal_envelope_frame(recipient_path: &str, manifest: Option<&str>) -> WireFrame {
  WireFrame::Envelope(EnvelopePdu::new(
    recipient_path.to_string(),
    None,
    1,
    2,
    1,
    EnvelopePayload::new(7, manifest.map(ToString::to_string), Bytes::from_static(b"hello")),
  ))
}

fn advertisement_frame(table_kind: CompressionTableKind, generation: u64, entry_id: u32, literal: &str) -> WireFrame {
  WireFrame::Control(ControlPdu::CompressionAdvertisement {
    authority: "remote@host:1".to_string(),
    table_kind,
    generation,
    entries: vec![CompressionTableEntry::new(entry_id, literal.to_string())],
  })
}

fn ack_frame(table_kind: CompressionTableKind, generation: u64) -> WireFrame {
  WireFrame::Control(ControlPdu::CompressionAck { authority: "remote@host:1".to_string(), table_kind, generation })
}

fn advertisement_generation(frame: WireFrame) -> u64 {
  match frame {
    | WireFrame::Control(ControlPdu::CompressionAdvertisement { generation, .. }) => generation,
    | _ => panic!("expected compression advertisement"),
  }
}

#[test]
#[should_panic(expected = "expected compression advertisement")]
fn advertisement_generation_rejects_other_frames() {
  advertisement_generation(WireFrame::Control(ControlPdu::Heartbeat { authority: "remote@host:1".to_string() }));
}

#[test]
fn inbound_advertisement_updates_table_and_replies_with_ack() {
  let mut tables = TcpCompressionTables::new(RemoteCompressionConfig::new());

  let action = tables
    .handle_inbound_frame(advertisement_frame(CompressionTableKind::ActorRef, 7, 3, "/user/a"), "local@host:2")
    .unwrap();

  assert!(matches!(
    action,
    InboundCompressionAction::Reply(ControlPdu::CompressionAck {
      authority,
      table_kind: CompressionTableKind::ActorRef,
      generation: 7,
    }) if authority == "local@host:2"
  ));

  let action =
    tables.handle_inbound_frame(envelope_frame(CompressedText::table_ref(3), None, None), "local@host:2").unwrap();

  assert!(matches!(
    action,
    InboundCompressionAction::Forward(WireFrame::Envelope(pdu)) if pdu.recipient_path() == "/user/a"
  ));
}

#[test]
fn inbound_manifest_advertisement_resolves_manifest_metadata() {
  let mut tables = TcpCompressionTables::new(RemoteCompressionConfig::new());

  let action = tables
    .handle_inbound_frame(advertisement_frame(CompressionTableKind::Manifest, 8, 4, "example.Manifest"), "local@host:2")
    .unwrap();
  assert!(matches!(
    action,
    InboundCompressionAction::Reply(ControlPdu::CompressionAck {
      authority,
      table_kind: CompressionTableKind::Manifest,
      generation: 8,
    }) if authority == "local@host:2"
  ));

  let action = tables
    .handle_inbound_frame(
      envelope_frame(CompressedText::literal("/user/a".to_string()), None, Some(CompressedText::table_ref(4))),
      "local@host:2",
    )
    .unwrap();

  assert!(matches!(
    action,
    InboundCompressionAction::Forward(WireFrame::Envelope(pdu)) if pdu.manifest() == Some("example.Manifest")
  ));
}

#[test]
fn outbound_acknowledged_metadata_uses_table_refs() {
  let mut tables = TcpCompressionTables::new(RemoteCompressionConfig::new());

  let first = tables.apply_outbound_frame(literal_envelope_frame("/user/a", Some("example.Manifest")));
  assert!(matches!(
    first,
    WireFrame::Envelope(pdu)
      if pdu.recipient_path_metadata().as_literal() == Some("/user/a")
        && pdu.manifest_metadata().and_then(CompressedText::as_literal) == Some("example.Manifest")
  ));

  let actor_generation = advertisement_generation(
    tables
      .create_advertisement(CompressionTableKind::ActorRef, "local@host:2")
      .expect("actor-ref advertisement should be created"),
  );
  let manifest_generation = advertisement_generation(
    tables
      .create_advertisement(CompressionTableKind::Manifest, "local@host:2")
      .expect("manifest advertisement should be created"),
  );
  assert!(matches!(
    tables.handle_inbound_frame(ack_frame(CompressionTableKind::ActorRef, actor_generation), "local@host:2"),
    Ok(InboundCompressionAction::Consumed)
  ));
  assert!(matches!(
    tables.handle_inbound_frame(ack_frame(CompressionTableKind::Manifest, manifest_generation), "local@host:2"),
    Ok(InboundCompressionAction::Consumed)
  ));

  let second = tables.apply_outbound_frame(literal_envelope_frame("/user/a", Some("example.Manifest")));

  assert!(matches!(
    second,
    WireFrame::Envelope(pdu)
      if pdu.recipient_path_metadata().as_table_ref().is_some()
        && pdu.manifest_metadata().and_then(CompressedText::as_table_ref).is_some()
        && pdu.payload() == &Bytes::from_static(b"hello")
  ));
}

#[test]
fn unknown_inbound_reference_is_rejected() {
  let mut tables = TcpCompressionTables::new(RemoteCompressionConfig::new());

  let err =
    tables.handle_inbound_frame(envelope_frame(CompressedText::table_ref(9), None, None), "local@host:2").unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn disabled_config_keeps_literals_and_does_not_ack_advertisements() {
  let config = RemoteCompressionConfig::new().with_actor_ref_max(None).with_manifest_max(max(1));
  let mut tables = TcpCompressionTables::new(config);

  let frame = tables.apply_outbound_frame(literal_envelope_frame("/user/a", None));
  assert!(matches!(
    frame,
    WireFrame::Envelope(pdu) if pdu.recipient_path_metadata().as_literal() == Some("/user/a")
  ));
  assert!(tables.create_advertisement(CompressionTableKind::ActorRef, "local@host:2").is_none());

  let action = tables
    .handle_inbound_frame(advertisement_frame(CompressionTableKind::ActorRef, 7, 3, "/user/a"), "local@host:2")
    .unwrap();
  assert!(matches!(action, InboundCompressionAction::Consumed));
}

#[test]
fn table_ref_metadata_is_preserved_without_observation() {
  let mut table = CompressionTable::new(max(1));

  let metadata = observe_and_encode(&mut table, &CompressedText::table_ref(3));

  assert_eq!(metadata.as_table_ref(), Some(3));
}

#[tokio::test]
async fn compression_advertisement_interval_follows_config() {
  assert!(compression_advertisement_interval(max(1), Duration::from_millis(1)).is_some());
  assert!(compression_advertisement_interval(None, Duration::from_millis(1)).is_none());
}

#[tokio::test]
async fn disabled_compression_advertisement_tick_remains_pending() {
  let mut interval = None;

  let result = tokio::time::timeout(Duration::from_millis(1), next_compression_advertisement_tick(&mut interval)).await;

  assert!(result.is_err());
}
