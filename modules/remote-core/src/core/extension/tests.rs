use alloc::{collections::VecDeque, string::String, vec, vec::Vec};
use core::{
  any::type_name,
  future::{Future, ready},
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};
use std::{
  sync::Arc,
  task::{Context, Poll, Wake, Waker},
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::CorrelationId,
  system::{
    ActorSystem,
    state::{SystemStateShared, system_state::SystemState},
  },
};
use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{Association, QuarantineReason},
  config::RemoteConfig,
  envelope::{InboundEnvelope, OutboundEnvelope, OutboundPriority},
  extension::{
    EventPublisher, Remote, RemoteActorRefResolveCacheEvent, RemoteActorRefResolveCacheOutcome,
    RemoteAuthoritySnapshot, RemoteEvent, RemoteEventReceiver, Remoting, RemotingError, RemotingLifecycleState,
  },
  instrument::{HandshakePhase, RemoteInstrument},
  transport::{BackpressureSignal, RemoteTransport, TransportEndpoint, TransportError},
  wire::{HandshakePdu, HandshakeRsp},
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

impl VecRemoteEventReceiver {
  fn new(events: impl IntoIterator<Item = RemoteEvent>) -> Self {
    Self { events: events.into_iter().collect() }
  }
}

impl RemoteEventReceiver for VecRemoteEventReceiver {
  fn recv(&mut self) -> impl Future<Output = Option<RemoteEvent>> + Send + '_ {
    ready(self.events.pop_front())
  }
}

struct NoopWaker;

struct CountingInstrument {
  send_calls:      ArcShared<AtomicUsize>,
  handshake_calls: ArcShared<AtomicUsize>,
}

impl CountingInstrument {
  fn new(send_calls: ArcShared<AtomicUsize>, handshake_calls: ArcShared<AtomicUsize>) -> Self {
    Self { send_calls, handshake_calls }
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

fn active_association(local: Address, remote: Address, config: &RemoteConfig) -> Association {
  let mut association = Association::from_config(UniqueAddress::new(local, 1), remote.clone(), config);
  let start_effects = association.associate(TransportEndpoint::new(remote.to_string()), 1);
  assert_eq!(start_effects.len(), 1);
  let accepted_effects = association
    .accept_handshake_response(&HandshakeRsp::new(UniqueAddress::new(remote, 2)), 2)
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
  let remote = Remote::new(RecordingTransport::new(vec![address]), RemoteConfig::new("127.0.0.1"), event_publisher());
  let mut receiver = VecRemoteEventReceiver::new([]);

  assert_eq!(block_on_ready(remote.run(&mut receiver)).unwrap_err(), RemotingError::EventReceiverClosed);
}

#[test]
fn run_returns_ok_on_transport_shutdown_event() {
  let address = Address::new("sys", "127.0.0.1", 2552);
  let remote = Remote::new(RecordingTransport::new(vec![address]), RemoteConfig::new("127.0.0.1"), event_publisher());
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
fn run_returns_unimplemented_event_for_unwired_remote_events() {
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let events = [
    RemoteEvent::InboundFrameReceived { authority: endpoint.clone(), frame: vec![1, 2, 3] },
    RemoteEvent::HandshakeTimerFired { authority: endpoint.clone(), generation: 1 },
    RemoteEvent::ConnectionLost { authority: endpoint, cause: TransportError::ConnectionClosed },
  ];

  for event in events {
    let address = Address::new("sys", "127.0.0.1", 2552);
    let remote = Remote::new(RecordingTransport::new(vec![address]), RemoteConfig::new("127.0.0.1"), event_publisher());
    let mut receiver = VecRemoteEventReceiver::new([event]);

    assert_eq!(block_on_ready(remote.run(&mut receiver)).unwrap_err(), RemotingError::UnimplementedEvent);
  }
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
