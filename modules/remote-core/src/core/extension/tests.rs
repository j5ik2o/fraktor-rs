use alloc::{collections::VecDeque, string::String, vec, vec::Vec};
use core::{
  any::type_name,
  future::Future,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};
use std::{
  sync::Arc,
  task::{Context, Poll, Wake, Waker},
};

use bytes::{Bytes, BytesMut};
use fraktor_actor_core_rs::core::kernel::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::CorrelationId,
  system::{
    ActorSystem,
    state::{SystemStateShared, system_state::SystemState},
  },
};
use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::core::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{Association, AssociationEffect, QuarantineReason},
  config::RemoteConfig,
  envelope::{InboundEnvelope, OutboundEnvelope, OutboundPriority},
  extension::{
    EventPublisher, Remote, RemoteActorRefResolveCacheEvent, RemoteActorRefResolveCacheOutcome,
    RemoteAuthoritySnapshot, RemoteEvent, RemoteEventReceiver, RemoteShared, Remoting, RemotingError,
    RemotingLifecycleState,
  },
  instrument::{FlightRecorderEvent, HandshakePhase, NoopInstrument, RemoteInstrument, RemotingFlightRecorder},
  transport::{BackpressureSignal, RemoteTransport, TransportEndpoint, TransportError},
  wire::{
    Codec, ControlCodec, ControlPdu, EnvelopeCodec, EnvelopePdu, HandshakeCodec, HandshakePdu, HandshakeReq,
    HandshakeRsp,
  },
};

struct RecordingTransport {
  addresses: Vec<Address>,
  start_result: Result<(), TransportError>,
  shutdown_result: Result<(), TransportError>,
  send_result: Result<(), TransportError>,
  running: bool,
  shutdown_calls: ArcShared<AtomicUsize>,
  send_calls: ArcShared<AtomicUsize>,
  handshake_calls: ArcShared<AtomicUsize>,
  timeout_calls: ArcShared<AtomicUsize>,
  timeout_before_handshake_calls: ArcShared<AtomicUsize>,
}

struct VecRemoteEventReceiver {
  events: VecDeque<RemoteEvent>,
}

struct PendingRemoteEventReceiver;

impl VecRemoteEventReceiver {
  fn new(events: impl IntoIterator<Item = RemoteEvent>) -> Self {
    Self { events: events.into_iter().collect() }
  }
}

impl RemoteEventReceiver for VecRemoteEventReceiver {
  fn poll_recv(&mut self, _cx: &mut Context<'_>) -> Poll<Option<RemoteEvent>> {
    Poll::Ready(self.events.pop_front())
  }
}

impl RemoteEventReceiver for PendingRemoteEventReceiver {
  fn poll_recv(&mut self, _cx: &mut Context<'_>) -> Poll<Option<RemoteEvent>> {
    Poll::Pending
  }
}

struct NoopWaker;

struct CountingInstrument {
  send_calls:      ArcShared<AtomicUsize>,
  handshake_calls: ArcShared<AtomicUsize>,
}

struct SharedRecorderInstrument {
  recorder: SharedLock<RemotingFlightRecorder>,
}

impl CountingInstrument {
  fn new(send_calls: ArcShared<AtomicUsize>, handshake_calls: ArcShared<AtomicUsize>) -> Self {
    Self { send_calls, handshake_calls }
  }
}

impl SharedRecorderInstrument {
  fn new(recorder: SharedLock<RemotingFlightRecorder>) -> Self {
    Self { recorder }
  }
}

impl RemoteInstrument for CountingInstrument {
  fn on_send(&mut self, _envelope: &OutboundEnvelope, _now_ms: u64) {
    self.send_calls.fetch_add(1, Ordering::Relaxed);
  }

  fn on_receive(&mut self, _envelope: &InboundEnvelope, _now_ms: u64) {}

  fn record_handshake(&mut self, _authority: &TransportEndpoint, phase: HandshakePhase, _now_ms: u64) {
    if phase == HandshakePhase::Started {
      self.handshake_calls.fetch_add(1, Ordering::Relaxed);
    }
  }

  fn record_quarantine(&mut self, _authority: &TransportEndpoint, _reason: &QuarantineReason, _now_ms: u64) {}

  fn record_backpressure(
    &mut self,
    _authority: &TransportEndpoint,
    _signal: BackpressureSignal,
    _correlation_id: CorrelationId,
    _now_ms: u64,
  ) {
  }
}

impl RemoteInstrument for SharedRecorderInstrument {
  fn on_send(&mut self, envelope: &OutboundEnvelope, now_ms: u64) {
    self.recorder.with_lock(|recorder| recorder.on_send(envelope, now_ms));
  }

  fn on_receive(&mut self, envelope: &InboundEnvelope, now_ms: u64) {
    self.recorder.with_lock(|recorder| recorder.on_receive(envelope, now_ms));
  }

  fn record_handshake(&mut self, authority: &TransportEndpoint, phase: HandshakePhase, now_ms: u64) {
    self.recorder.with_lock(|recorder| RemoteInstrument::record_handshake(recorder, authority, phase, now_ms));
  }

  fn record_quarantine(&mut self, authority: &TransportEndpoint, reason: &QuarantineReason, now_ms: u64) {
    self.recorder.with_lock(|recorder| RemoteInstrument::record_quarantine(recorder, authority, reason, now_ms));
  }

  fn record_backpressure(
    &mut self,
    authority: &TransportEndpoint,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
    now_ms: u64,
  ) {
    self
      .recorder
      .with_lock(|recorder| RemoteInstrument::record_backpressure(recorder, authority, signal, correlation_id, now_ms));
  }
}

impl Wake for NoopWaker {
  fn wake(self: Arc<Self>) {}
}

fn block_on_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::from(Arc::new(NoopWaker));
  let mut context = Context::from_waker(&waker);
  let mut future = Box::pin(future);
  match future.as_mut().poll(&mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!(
      "test future was still pending after one poll with a NoopWaker; future_type={}; add a ready receiver event or drive the async dependency explicitly",
      type_name::<F>()
    ),
  }
}

impl RecordingTransport {
  fn new(addresses: Vec<Address>) -> Self {
    Self {
      addresses,
      start_result: Ok(()),
      shutdown_result: Ok(()),
      send_result: Ok(()),
      running: false,
      shutdown_calls: ArcShared::new(AtomicUsize::new(0)),
      send_calls: ArcShared::new(AtomicUsize::new(0)),
      handshake_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_before_handshake_calls: ArcShared::new(AtomicUsize::new(0)),
    }
  }

  fn with_shutdown_result(addresses: Vec<Address>, shutdown_result: Result<(), TransportError>) -> Self {
    Self {
      addresses,
      start_result: Ok(()),
      shutdown_result,
      send_result: Ok(()),
      running: false,
      shutdown_calls: ArcShared::new(AtomicUsize::new(0)),
      send_calls: ArcShared::new(AtomicUsize::new(0)),
      handshake_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_before_handshake_calls: ArcShared::new(AtomicUsize::new(0)),
    }
  }

  fn with_start_result(
    addresses: Vec<Address>,
    start_result: Result<(), TransportError>,
  ) -> (ArcShared<AtomicUsize>, Self) {
    let shutdown_calls = ArcShared::new(AtomicUsize::new(0));
    (shutdown_calls.clone(), Self {
      addresses,
      start_result,
      shutdown_result: Ok(()),
      send_result: Ok(()),
      running: false,
      shutdown_calls,
      send_calls: ArcShared::new(AtomicUsize::new(0)),
      handshake_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_before_handshake_calls: ArcShared::new(AtomicUsize::new(0)),
    })
  }

  fn with_send_result(addresses: Vec<Address>, send_result: Result<(), TransportError>) -> Self {
    Self {
      addresses,
      start_result: Ok(()),
      shutdown_result: Ok(()),
      send_result,
      running: false,
      shutdown_calls: ArcShared::new(AtomicUsize::new(0)),
      send_calls: ArcShared::new(AtomicUsize::new(0)),
      handshake_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_calls: ArcShared::new(AtomicUsize::new(0)),
      timeout_before_handshake_calls: ArcShared::new(AtomicUsize::new(0)),
    }
  }
}

impl RemoteTransport for RecordingTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    if self.start_result.is_ok() {
      self.running = true;
    }
    self.start_result.clone()
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    self.shutdown_calls.fetch_add(1, Ordering::Relaxed);
    if self.shutdown_result.is_ok() {
      self.running = false;
    }
    self.shutdown_result.clone()
  }

  fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    self.send_calls.fetch_add(1, Ordering::Relaxed);
    if !self.running {
      return Err((TransportError::NotStarted, Box::new(envelope)));
    }
    match self.send_result.clone() {
      | Ok(()) => Ok(()),
      | Err(err) => Err((err, Box::new(envelope))),
    }
  }

  fn send_handshake(&mut self, _remote: &Address, _pdu: HandshakePdu) -> Result<(), TransportError> {
    self.handshake_calls.fetch_add(1, Ordering::Relaxed);
    if self.running { Ok(()) } else { Err(TransportError::NotStarted) }
  }

  fn schedule_handshake_timeout(
    &mut self,
    _authority: &TransportEndpoint,
    _timeout: Duration,
    _generation: u64,
  ) -> Result<(), TransportError> {
    if self.handshake_calls.load(Ordering::Relaxed) == 0 {
      self.timeout_before_handshake_calls.fetch_add(1, Ordering::Relaxed);
    }
    self.timeout_calls.fetch_add(1, Ordering::Relaxed);
    if self.running { Ok(()) } else { Err(TransportError::NotStarted) }
  }

  fn addresses(&self) -> &[Address] {
    &self.addresses
  }

  fn default_address(&self) -> Option<&Address> {
    self.addresses.first()
  }

  fn local_address_for_remote(&self, _remote: &Address) -> Option<&Address> {
    self.default_address()
  }

  fn quarantine(
    &mut self,
    _address: &Address,
    _uid: Option<u64>,
    _reason: QuarantineReason,
  ) -> Result<(), TransportError> {
    if self.running { Ok(()) } else { Err(TransportError::NotStarted) }
  }
}

fn event_publisher() -> EventPublisher {
  let system = ActorSystem::from_state(SystemStateShared::new(SystemState::new()));
  EventPublisher::new(system.downgrade())
}

fn encode_envelope_pdu(pdu: &EnvelopePdu) -> Vec<u8> {
  let mut buffer = BytesMut::new();
  EnvelopeCodec::new().encode(pdu, &mut buffer).expect("envelope pdu should encode");
  buffer.freeze().to_vec()
}

fn encode_control_pdu(pdu: &ControlPdu) -> Vec<u8> {
  let mut buffer = BytesMut::new();
  ControlCodec::new().encode(pdu, &mut buffer).expect("control pdu should encode");
  buffer.freeze().to_vec()
}

fn encode_handshake_pdu(pdu: &HandshakePdu) -> Vec<u8> {
  let mut buffer = BytesMut::new();
  HandshakeCodec::new().encode(pdu, &mut buffer).expect("handshake pdu should encode");
  buffer.freeze().to_vec()
}

fn active_association(local: Address, remote: Address, config: &RemoteConfig) -> Association {
  let mut association = Association::from_config(UniqueAddress::new(local, 1), remote.clone(), config);
  let mut instrument = NoopInstrument;
  let start_effects = association.associate(TransportEndpoint::new(remote.to_string()), 1, &mut instrument);
  assert_eq!(start_effects.len(), 1);
  let accepted_effects = association
    .accept_handshake_response(&HandshakeRsp::new(UniqueAddress::new(remote, 2)), 2, &mut instrument)
    .expect("matching handshake response should activate association");
  assert!(!accepted_effects.is_empty());
  association
}

// ---------------------------------------------------------------------------
// RemotingLifecycleState — happy paths
// ---------------------------------------------------------------------------

#[test]
fn new_state_is_pending() {
  let s = RemotingLifecycleState::new();
  assert!(!s.is_running());
  assert!(!s.is_terminated());
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

#[test]
fn pending_to_starting_to_running_to_shuttingdown_to_shutdown() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap(); // Pending → Starting
  assert!(!s.is_running(), "Starting is not Running yet");
  s.mark_started().unwrap(); // Starting → Running
  assert!(s.is_running());
  s.ensure_running().unwrap();
  s.transition_to_shutdown().unwrap(); // Running → ShuttingDown
  assert!(!s.is_running());
  s.mark_shutdown().unwrap(); // ShuttingDown → Shutdown
  assert!(s.is_terminated());
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

#[test]
fn pending_can_shortcut_to_shutdown() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_shutdown().unwrap(); // Pending → Shutdown
  assert!(s.is_terminated());
}

#[test]
fn start_failure_rolls_back_to_pending() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_start_failed().unwrap();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  assert!(s.is_running());
}

#[test]
fn remote_start_failure_attempts_transport_cleanup_and_rolls_back() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let (shutdown_calls, transport) =
    RecordingTransport::with_start_result(vec![address], Err(TransportError::NotAvailable));
  let mut remote = Remote::new(transport, RemoteConfig::new("127.0.0.1"), event_publisher());

  assert_eq!(remote.start().unwrap_err(), RemotingError::TransportUnavailable);

  assert_eq!(shutdown_calls.load(Ordering::Relaxed), 1);
  assert!(remote.addresses().is_empty());
  assert_eq!(remote.lifecycle().ensure_running().unwrap_err(), RemotingError::NotStarted);
}

#[test]
fn shutdown_failure_rolls_back_to_running() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  s.transition_to_shutdown().unwrap();
  s.mark_shutdown_failed().unwrap();

  assert!(s.is_running());
  s.ensure_running().unwrap();
}

#[test]
fn remote_shutdown_clears_advertised_addresses_after_success() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let mut remote =
    Remote::new(RecordingTransport::new(vec![address.clone()]), RemoteConfig::new("127.0.0.1"), event_publisher());

  remote.start().unwrap();
  assert_eq!(remote.addresses(), [address.clone()].as_slice());

  remote.shutdown().unwrap();

  assert!(remote.addresses().is_empty());
  assert!(remote.lifecycle().is_terminated());
}

#[test]
fn remote_shutdown_failure_keeps_advertised_addresses() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let mut remote = Remote::new(
    RecordingTransport::with_shutdown_result(vec![address.clone()], Err(TransportError::NotAvailable)),
    RemoteConfig::new("127.0.0.1"),
    event_publisher(),
  );

  remote.start().unwrap();
  assert_eq!(remote.shutdown().unwrap_err(), RemotingError::TransportUnavailable);

  assert_eq!(remote.addresses(), [address].as_slice());
  assert!(remote.lifecycle().is_running());
}

#[test]
fn run_returns_error_when_receiver_is_closed() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let mut remote =
    Remote::new(RecordingTransport::new(vec![address]), RemoteConfig::new("127.0.0.1"), event_publisher());
  let mut receiver = VecRemoteEventReceiver::new([]);

  assert_eq!(block_on_ready(remote.run(&mut receiver)).unwrap_err(), RemotingError::EventReceiverClosed);
}

#[test]
fn run_returns_ok_on_transport_shutdown_event() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let mut remote =
    Remote::new(RecordingTransport::new(vec![address]), RemoteConfig::new("127.0.0.1"), event_publisher());
  let mut receiver = VecRemoteEventReceiver::new([RemoteEvent::TransportShutdown]);

  block_on_ready(remote.run(&mut receiver)).unwrap();
}

#[test]
fn run_sends_outbound_enqueued_event_and_records_instrument() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let transport = RecordingTransport::new(vec![local_address.clone()]);
  let transport_send_calls = transport.send_calls.clone();
  let handshake_send_calls = transport.handshake_calls.clone();
  let timeout_calls = transport.timeout_calls.clone();
  let timeout_before_handshake_calls = transport.timeout_before_handshake_calls.clone();
  let instrument_send_calls = ArcShared::new(AtomicUsize::new(0));
  let instrument_handshake_calls = ArcShared::new(AtomicUsize::new(0));
  let instrument = CountingInstrument::new(instrument_send_calls.clone(), instrument_handshake_calls.clone());
  let mut remote = Remote::with_instrument(transport, config.clone(), event_publisher(), Box::new(instrument));
  remote.start().expect("remote should start before outbound delivery");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let recipient = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("recipient path");
  let envelope = OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  );
  let event = RemoteEvent::OutboundEnqueued {
    authority: TransportEndpoint::new(remote_address.to_string()),
    envelope:  Box::new(envelope),
    now_ms:    42,
  };
  let mut receiver = VecRemoteEventReceiver::new([event, RemoteEvent::TransportShutdown]);

  block_on_ready(remote.run(&mut receiver)).unwrap();

  assert_eq!(transport_send_calls.load(Ordering::Relaxed), 1);
  assert_eq!(handshake_send_calls.load(Ordering::Relaxed), 0);
  assert_eq!(timeout_calls.load(Ordering::Relaxed), 0);
  assert_eq!(timeout_before_handshake_calls.load(Ordering::Relaxed), 0);
  assert_eq!(instrument_send_calls.load(Ordering::Relaxed), 1);
  assert_eq!(instrument_handshake_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn remote_new_defaults_to_noop_and_flight_recorder_instrument_records_send_hook() {
  let noop_address = Address::new("sys", "127.0.0.1", 2552);
  let mut noop_remote =
    Remote::new(RecordingTransport::new(vec![noop_address]), RemoteConfig::new("127.0.0.1"), event_publisher());
  let mut noop_receiver = VecRemoteEventReceiver::new([RemoteEvent::TransportShutdown]);
  block_on_ready(noop_remote.run(&mut noop_receiver)).expect("NoopInstrument should accept event loop hooks");

  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let recorder = SharedLock::new_with_driver::<DefaultMutex<_>>(RemotingFlightRecorder::new(8));
  let instrument = SharedRecorderInstrument::new(recorder.clone());
  let mut remote = Remote::with_instrument(
    RecordingTransport::new(vec![local_address.clone()]),
    config.clone(),
    event_publisher(),
    Box::new(instrument),
  );
  remote.start().expect("remote should start before outbound delivery");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let recipient = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("recipient path");
  let envelope = OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  );
  let mut receiver = VecRemoteEventReceiver::new([
    RemoteEvent::OutboundEnqueued {
      authority: TransportEndpoint::new(remote_address.to_string()),
      envelope:  Box::new(envelope),
      now_ms:    42,
    },
    RemoteEvent::TransportShutdown,
  ]);

  block_on_ready(remote.run(&mut receiver)).expect("flight recorder instrument should not fail the event loop");

  let snapshot = recorder.with_lock(|recorder| recorder.snapshot());
  assert!(matches!(
    snapshot.events(),
    [
      FlightRecorderEvent::Send {
        authority,
        priority: 1,
        now_ms: 42,
        ..
      }
    ] if authority == "remote-sys@10.0.0.1:2552"
  ));
}

#[test]
fn run_continues_event_loop_after_outbound_send_failure() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let transport = RecordingTransport::with_send_result(vec![local_address.clone()], Err(TransportError::NotAvailable));
  let transport_send_calls = transport.send_calls.clone();
  let mut remote = Remote::new(transport, config.clone(), event_publisher());
  remote.start().expect("remote should start before outbound delivery");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let recipient = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("recipient path");
  let envelope = OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  );
  let outbound_event = RemoteEvent::OutboundEnqueued {
    authority: TransportEndpoint::new(remote_address.to_string()),
    envelope:  Box::new(envelope),
    now_ms:    42,
  };
  let mut receiver = VecRemoteEventReceiver::new([outbound_event, RemoteEvent::TransportShutdown]);

  // 単一 envelope の送信失敗で event loop 全体が落ちると、後続の TransportShutdown
  // を処理できず error 扱いになる。修正後は失敗した envelope を association に戻し
  // event loop は継続するため、TransportShutdown が正常に処理されて Ok で抜ける。
  block_on_ready(remote.run(&mut receiver)).unwrap();

  // 失敗した envelope は association に戻されるため、drain は break して send は 1 回のみ。
  assert_eq!(transport_send_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn run_does_not_send_outbound_enqueued_event_before_association_is_active() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let transport = RecordingTransport::new(vec![local_address]);
  let transport_send_calls = transport.send_calls.clone();
  let handshake_send_calls = transport.handshake_calls.clone();
  let timeout_calls = transport.timeout_calls.clone();
  let timeout_before_handshake_calls = transport.timeout_before_handshake_calls.clone();
  let instrument_send_calls = ArcShared::new(AtomicUsize::new(0));
  let instrument_handshake_calls = ArcShared::new(AtomicUsize::new(0));
  let instrument = CountingInstrument::new(instrument_send_calls.clone(), instrument_handshake_calls.clone());
  let mut remote =
    Remote::with_instrument(transport, RemoteConfig::new("127.0.0.1"), event_publisher(), Box::new(instrument));
  remote.start().expect("remote should start before outbound delivery");
  let recipient = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("recipient path");
  let envelope = OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  );
  let event = RemoteEvent::OutboundEnqueued {
    authority: TransportEndpoint::new(remote_address.to_string()),
    envelope:  Box::new(envelope),
    now_ms:    42,
  };
  let mut receiver = VecRemoteEventReceiver::new([event, RemoteEvent::TransportShutdown]);

  block_on_ready(remote.run(&mut receiver)).unwrap();

  assert_eq!(transport_send_calls.load(Ordering::Relaxed), 0);
  assert_eq!(handshake_send_calls.load(Ordering::Relaxed), 1);
  assert_eq!(timeout_calls.load(Ordering::Relaxed), 1);
  assert_eq!(timeout_before_handshake_calls.load(Ordering::Relaxed), 0);
  assert_eq!(instrument_send_calls.load(Ordering::Relaxed), 0);
  assert_eq!(instrument_handshake_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn run_returns_codec_failed_for_invalid_inbound_frame() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let mut remote =
    Remote::new(RecordingTransport::new(vec![address]), RemoteConfig::new("127.0.0.1"), event_publisher());
  remote.start().expect("remote should be running before inbound frame dispatch");
  let event = RemoteEvent::InboundFrameReceived {
    authority: TransportEndpoint::new("remote-sys@10.0.0.1:2552"),
    frame:     vec![1, 2, 3],
    now_ms:    1,
  };
  let mut receiver = VecRemoteEventReceiver::new([event]);

  assert_eq!(block_on_ready(remote.run(&mut receiver)).unwrap_err(), RemotingError::CodecFailed);
}

#[test]
fn inbound_handshake_request_rejects_forged_local_destination() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let transport = RecordingTransport::new(vec![local_address]);
  let handshake_calls = transport.handshake_calls.clone();
  let mut remote = Remote::new(transport, RemoteConfig::new("127.0.0.1"), event_publisher());
  remote.start().expect("remote should be running before inbound handshake");
  let forged_local = Address::new("sys", "127.0.0.2", 2552);
  let request = HandshakePdu::Req(HandshakeReq::new(UniqueAddress::new(remote_address.clone(), 7), forged_local));

  remote
    .handle_remote_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(remote_address.to_string()),
      frame:     encode_handshake_pdu(&request),
      now_ms:    42,
    })
    .expect("forged destination should be ignored without failing the event loop");

  assert_eq!(handshake_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn inbound_envelope_is_buffered_for_local_delivery() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let mut remote = Remote::new(RecordingTransport::new(vec![local_address.clone()]), config.clone(), event_publisher());
  remote.start().expect("remote should be running before inbound envelope");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let pdu = EnvelopePdu::new(
    String::from("fraktor.tcp://sys@127.0.0.1:2552/user/local"),
    Some(String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/sender")),
    1,
    2,
    1,
    Bytes::from_static(b"inbound-payload"),
  );

  remote
    .handle_remote_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(remote_address.to_string()),
      frame:     encode_envelope_pdu(&pdu),
      now_ms:    55,
    })
    .expect("active inbound envelope should be accepted");

  let deliveries = remote.drain_inbound_envelopes();
  assert_eq!(deliveries.len(), 1);
  assert_eq!(deliveries[0].recipient().to_canonical_uri(), "fraktor.tcp://sys@127.0.0.1:2552/user/local");
  assert_eq!(deliveries[0].remote_node().system(), "remote-sys");
  assert_eq!(deliveries[0].message().downcast_ref::<Bytes>(), Some(&Bytes::from_static(b"inbound-payload")));
}

#[test]
fn inbound_senderless_envelope_matches_existing_association_by_peer_endpoint() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let mut remote = Remote::new(RecordingTransport::new(vec![local_address.clone()]), config.clone(), event_publisher());
  remote.start().expect("remote should be running before inbound envelope");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let pdu = EnvelopePdu::new(
    String::from("fraktor.tcp://sys@127.0.0.1:2552/user/local"),
    None,
    3,
    4,
    1,
    Bytes::from_static(b"senderless-payload"),
  );

  remote
    .handle_remote_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new("10.0.0.1:2552"),
      frame:     encode_envelope_pdu(&pdu),
      now_ms:    56,
    })
    .expect("senderless inbound envelope should match the existing peer association");

  let deliveries = remote.drain_inbound_envelopes();
  assert_eq!(deliveries.len(), 1);
  assert_eq!(deliveries[0].sender(), None);
  assert_eq!(deliveries[0].message().downcast_ref::<Bytes>(), Some(&Bytes::from_static(b"senderless-payload")));
}

#[test]
fn inbound_quarantine_control_quarantines_matching_association() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let recorder = SharedLock::new_with_driver::<DefaultMutex<_>>(RemotingFlightRecorder::new(4));
  let instrument = SharedRecorderInstrument::new(recorder.clone());
  let mut remote = Remote::with_instrument(
    RecordingTransport::new(vec![local_address.clone()]),
    config.clone(),
    event_publisher(),
    Box::new(instrument),
  );
  remote.start().expect("remote should be running before inbound control");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let pdu =
    ControlPdu::Quarantine { authority: remote_address.to_string(), reason: Some(String::from("remote says no")) };

  remote
    .handle_remote_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(remote_address.to_string()),
      frame:     encode_control_pdu(&pdu),
      now_ms:    70,
    })
    .expect("quarantine control should be applied");

  let snapshot = recorder.with_lock(|recorder| recorder.snapshot());
  assert!(matches!(
    snapshot.events(),
    [
      FlightRecorderEvent::Quarantine {
        authority,
        reason,
        now_ms: 70
      }
    ] if authority == "remote-sys@10.0.0.1:2552" && reason == "remote says no"
  ));
}

#[test]
fn inbound_shutdown_control_gates_matching_association() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let transport = RecordingTransport::new(vec![local_address.clone()]);
  let send_calls = transport.send_calls.clone();
  let handshake_calls = transport.handshake_calls.clone();
  let timeout_calls = transport.timeout_calls.clone();
  let mut remote = Remote::new(transport, config.clone(), event_publisher());
  remote.start().expect("remote should be running before inbound control");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let pdu = ControlPdu::Shutdown { authority: remote_address.to_string() };

  remote
    .handle_remote_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(remote_address.to_string()),
      frame:     encode_control_pdu(&pdu),
      now_ms:    80,
    })
    .expect("shutdown control should be applied");

  let recipient = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("recipient path");
  let envelope = OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  );
  remote
    .handle_remote_event(RemoteEvent::OutboundEnqueued {
      authority: TransportEndpoint::new(remote_address.to_string()),
      envelope:  Box::new(envelope),
      now_ms:    81,
    })
    .expect("gated association should keep outbound deferred");

  assert_eq!(send_calls.load(Ordering::Relaxed), 0);
  assert_eq!(handshake_calls.load(Ordering::Relaxed), 1);
  assert_eq!(timeout_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn outbound_high_watermark_notification_does_not_pause_internal_drain() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1").with_outbound_watermarks(1, 2);
  let transport = RecordingTransport::with_send_result(vec![local_address.clone()], Err(TransportError::SendFailed));
  let send_calls = transport.send_calls.clone();
  let mut remote = Remote::new(transport, config.clone(), event_publisher());
  remote.start().expect("remote should be running before outbound delivery");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let make_event = || {
    let recipient =
      ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("recipient path");
    let envelope = OutboundEnvelope::new(
      recipient,
      None,
      AnyMessage::new(String::from("payload")),
      OutboundPriority::User,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      CorrelationId::nil(),
    );
    RemoteEvent::OutboundEnqueued {
      authority: TransportEndpoint::new(remote_address.to_string()),
      envelope:  Box::new(envelope),
      now_ms:    90,
    }
  };

  remote.handle_remote_event(make_event()).expect("first enqueue should be handled");
  remote.handle_remote_event(make_event()).expect("second enqueue should be handled");
  remote.handle_remote_event(make_event()).expect("third enqueue should be handled");

  assert_eq!(send_calls.load(Ordering::Relaxed), 3);
}

#[test]
fn connection_lost_recovers_active_association() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let config = RemoteConfig::new("127.0.0.1");
  let transport = RecordingTransport::new(vec![local_address.clone()]);
  let handshake_send_calls = transport.handshake_calls.clone();
  let timeout_calls = transport.timeout_calls.clone();
  let mut remote = Remote::new(transport, config.clone(), event_publisher());
  remote.start().expect("remote should be running before connection loss");
  remote.insert_association(active_association(local_address, remote_address.clone(), &config));
  let event = RemoteEvent::ConnectionLost {
    authority: TransportEndpoint::new(remote_address.to_string()),
    cause:     TransportError::ConnectionClosed,
    now_ms:    42,
  };
  let mut receiver = VecRemoteEventReceiver::new([event, RemoteEvent::TransportShutdown]);

  block_on_ready(remote.run(&mut receiver)).unwrap();

  assert_eq!(handshake_send_calls.load(Ordering::Relaxed), 1);
  assert_eq!(timeout_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn handle_remote_event_ignores_handshake_timer_for_unknown_association() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let mut remote =
    Remote::new(RecordingTransport::new(vec![address]), RemoteConfig::new("127.0.0.1"), event_publisher());
  let event = RemoteEvent::HandshakeTimerFired {
    authority:  TransportEndpoint::new("remote-sys@10.0.0.1:2552"),
    generation: 1,
    now_ms:     1,
  };

  remote.handle_remote_event(event).expect("unknown timer should be ignored");
}

#[test]
fn handle_remote_event_discards_wrapped_stale_handshake_timer_generation() {
  let local_address = Address::new("sys", "127.0.0.1", 2552);
  let remote_address = Address::new("remote-sys", "10.0.0.1", 2552);
  let endpoint = TransportEndpoint::new(remote_address.to_string());
  let config = RemoteConfig::new("127.0.0.1");
  let recorder = SharedLock::new_with_driver::<DefaultMutex<_>>(RemotingFlightRecorder::new(4));
  let instrument = SharedRecorderInstrument::new(recorder.clone());
  let mut association =
    Association::from_config(UniqueAddress::new(local_address.clone(), 1), remote_address.clone(), &config);
  association.set_handshake_generation_for_test(u64::MAX);
  let effects = association.associate(endpoint.clone(), 10, &mut NoopInstrument);
  assert_eq!(association.handshake_generation(), 0);
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { generation: 0, .. }]));
  let mut remote = Remote::with_instrument(
    RecordingTransport::new(vec![local_address]),
    config,
    event_publisher(),
    Box::new(instrument),
  );
  remote.insert_association(association);

  remote
    .handle_remote_event(RemoteEvent::HandshakeTimerFired {
      authority:  endpoint.clone(),
      generation: u64::MAX,
      now_ms:     100,
    })
    .expect("stale wrapped timer should be discarded");
  remote
    .handle_remote_event(RemoteEvent::HandshakeTimerFired { authority: endpoint, generation: 0, now_ms: 101 })
    .expect("current wrapped timer should be handled");

  let snapshot = recorder.with_lock(|recorder| recorder.snapshot());
  assert!(matches!(
    snapshot.events(),
    [
      FlightRecorderEvent::Handshake {
        authority,
        phase: HandshakePhase::Rejected,
        now_ms: 101
      }
    ] if authority == "remote-sys@10.0.0.1:2552"
  ));
}

#[test]
fn remote_shared_remoting_methods_delegate_to_remote() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let shared = RemoteShared::new(Remote::new(
    RecordingTransport::new(vec![address.clone()]),
    RemoteConfig::new("127.0.0.1"),
    event_publisher(),
  ));

  shared.start().expect("shared start");

  assert_eq!(shared.addresses(), vec![address.clone()]);
  shared.quarantine(&address, Some(1), QuarantineReason::new("shared")).expect("shared quarantine");
  shared.shutdown().expect("shared shutdown");
  shared.shutdown().expect("second shared shutdown should be idempotent after termination");
}

#[test]
fn remote_shared_clones_observe_same_remote_state() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let starter = RemoteShared::new(Remote::new(
    RecordingTransport::new(vec![address.clone()]),
    RemoteConfig::new("127.0.0.1"),
    event_publisher(),
  ));
  let reader = starter.clone();

  starter.start().expect("start through first clone");

  assert_eq!(reader.addresses(), vec![address]);
  reader.shutdown().expect("shutdown through second clone");
}

#[test]
fn remote_shared_run_returns_ok_after_transport_shutdown_event() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let shared = RemoteShared::new(Remote::new(
    RecordingTransport::new(vec![address]),
    RemoteConfig::new("127.0.0.1"),
    event_publisher(),
  ));
  let mut receiver = VecRemoteEventReceiver::new([RemoteEvent::TransportShutdown]);

  block_on_ready(shared.run(&mut receiver)).expect("transport shutdown should stop shared run loop");
}

#[test]
fn remote_shared_run_returns_ready_after_shutdown_when_polled() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let shared = RemoteShared::new(Remote::new(
    RecordingTransport::new(vec![address]),
    RemoteConfig::new("127.0.0.1"),
    event_publisher(),
  ));
  shared.start().expect("shared remote should start");
  shared.shutdown().expect("standalone shutdown should update lifecycle");
  let mut receiver = PendingRemoteEventReceiver;
  let waker = Waker::from(Arc::new(NoopWaker));
  let mut context = Context::from_waker(&waker);
  let mut future = Box::pin(shared.run(&mut receiver));

  assert!(future.as_mut().poll(&mut context).is_ready());
}

// ---------------------------------------------------------------------------
// RemotingLifecycleState — invalid transitions
// ---------------------------------------------------------------------------

#[test]
fn mark_started_from_pending_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  assert_eq!(s.mark_started().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn mark_start_failed_from_pending_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  assert_eq!(s.mark_start_failed().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn transition_to_start_from_starting_is_already_running() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  assert_eq!(s.transition_to_start().unwrap_err(), RemotingError::AlreadyRunning);
}

#[test]
fn transition_to_start_from_running_is_already_running() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  assert_eq!(s.transition_to_start().unwrap_err(), RemotingError::AlreadyRunning);
}

#[test]
fn transition_to_start_from_shutdown_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_shutdown().unwrap(); // Pending → Shutdown
  assert_eq!(s.transition_to_start().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn transition_to_shutdown_from_starting_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  assert_eq!(s.transition_to_shutdown().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn transition_to_shutdown_from_shutdown_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_shutdown().unwrap();
  assert_eq!(s.transition_to_shutdown().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn mark_shutdown_from_running_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  assert_eq!(s.mark_shutdown().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn ensure_running_from_starting_returns_not_started() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

#[test]
fn ensure_running_from_shutting_down_returns_not_started() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  s.transition_to_shutdown().unwrap();
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

// ---------------------------------------------------------------------------
// RemoteAuthoritySnapshot
// ---------------------------------------------------------------------------

#[test]
fn remote_authority_snapshot_exposes_all_fields() {
  let addr = Address::new("sys", "host", 2552);
  let snap = RemoteAuthoritySnapshot::new(addr.clone(), true, false, Some(10_000), Some(String::from("fine")));
  assert_eq!(snap.address(), &addr);
  assert!(snap.is_connected());
  assert!(!snap.is_quarantined());
  assert_eq!(snap.last_contact_ms(), Some(10_000));
  assert_eq!(snap.quarantine_reason(), Some("fine"));
}

#[test]
fn remote_authority_snapshot_clone_preserves_fields() {
  let snap = RemoteAuthoritySnapshot::new(Address::new("sys", "host", 0), false, true, None, None);
  let cloned = snap.clone();
  assert_eq!(snap, cloned);
}

// ---------------------------------------------------------------------------
// RemoteActorRefResolveCacheEvent
// ---------------------------------------------------------------------------

#[test]
fn remote_actor_ref_resolve_cache_event_exposes_path_and_miss_outcome() {
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let event = RemoteActorRefResolveCacheEvent::new(path.clone(), RemoteActorRefResolveCacheOutcome::Miss);

  assert_eq!(event.path(), &path);
  assert_eq!(event.outcome(), RemoteActorRefResolveCacheOutcome::Miss);
}

#[test]
fn remote_actor_ref_resolve_cache_event_clone_preserves_hit_outcome() {
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let event = RemoteActorRefResolveCacheEvent::new(path.clone(), RemoteActorRefResolveCacheOutcome::Hit);

  let cloned = event.clone();

  assert_eq!(cloned.path(), &path);
  assert_eq!(cloned.outcome(), RemoteActorRefResolveCacheOutcome::Hit);
}
