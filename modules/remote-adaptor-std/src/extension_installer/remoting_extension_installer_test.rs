use std::{
  string::String,
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::ActorPathParser,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  event::stream::CorrelationId,
  serialization::{SerializationExtensionShared, default_serialization_extension_id},
};
use fraktor_remote_core_rs::{
  address::{RemoteNodeId, UniqueAddress},
  association::QuarantineReason,
  envelope::{OutboundEnvelope, OutboundPriority},
  transport::{TransportEndpoint, TransportError},
  wire::{AckPdu, HandshakePdu, HandshakeRsp},
};
use fraktor_utils_core_rs::sync::ArcShared;
use tokio::{
  sync::mpsc::{self, Receiver, Sender},
  time::{sleep, timeout},
};

use super::*;
use crate::extension_installer::flush_gate::StdFlushNotification;

struct TestRemoteTransport {
  addresses:    Vec<Address>,
  running:      bool,
  send_result:  Result<(), TransportError>,
  flush_result: Result<(), TransportError>,
}

impl TestRemoteTransport {
  fn new(addresses: Vec<Address>) -> Self {
    Self { addresses, running: false, send_result: Ok(()), flush_result: Ok(()) }
  }

  fn with_flush_result(addresses: Vec<Address>, flush_result: Result<(), TransportError>) -> Self {
    Self { addresses, running: false, send_result: Ok(()), flush_result }
  }
}

impl RemoteTransport for TestRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    if self.running {
      return Err(TransportError::AlreadyRunning);
    }
    self.running = true;
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    self.running = false;
    Ok(())
  }

  fn connect_peer(&mut self, _remote: &Address) -> Result<(), TransportError> {
    if self.running { Ok(()) } else { Err(TransportError::NotStarted) }
  }

  fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    if !self.running {
      return Err((TransportError::NotStarted, Box::new(envelope)));
    }
    match self.send_result.clone() {
      | Ok(()) => Ok(()),
      | Err(error) => Err((error, Box::new(envelope))),
    }
  }

  fn send_control(&mut self, _remote: &Address, _pdu: ControlPdu) -> Result<(), TransportError> {
    if self.running { Ok(()) } else { Err(TransportError::NotStarted) }
  }

  fn send_flush_request(&mut self, _remote: &Address, _pdu: ControlPdu, _lane_id: u32) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    self.flush_result.clone()
  }

  fn send_ack(&mut self, _remote: &Address, _pdu: AckPdu) -> Result<(), TransportError> {
    if self.running { Ok(()) } else { Err(TransportError::NotStarted) }
  }

  fn send_handshake(&mut self, _remote: &Address, _pdu: HandshakePdu) -> Result<(), TransportError> {
    if self.running { Ok(()) } else { Err(TransportError::NotStarted) }
  }

  fn schedule_handshake_timeout(
    &mut self,
    _authority: &TransportEndpoint,
    _timeout: Duration,
    _generation: u64,
  ) -> Result<(), TransportError> {
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

fn local_address() -> Address {
  Address::new("local-sys", "127.0.0.1", 2551)
}

fn remote_address() -> Address {
  Address::new("remote-sys", "10.0.0.1", 2552)
}

fn serialization_extension() -> ArcShared<SerializationExtensionShared> {
  let system = create_noop_actor_system();
  system.extended().register_extension(&default_serialization_extension_id())
}

fn remote_shared(config: RemoteConfig, transport: TestRemoteTransport) -> RemoteShared {
  let system = create_noop_actor_system();
  let remote = Remote::new(transport, config, EventPublisher::new(system.downgrade()), serialization_extension());
  let shared = RemoteShared::new(remote);
  shared.start().expect("remote should start");
  shared
}

fn remote_shared_not_started(config: RemoteConfig, transport: TestRemoteTransport) -> RemoteShared {
  let system = create_noop_actor_system();
  RemoteShared::new(Remote::new(transport, config, EventPublisher::new(system.downgrade()), serialization_extension()))
}

fn activate_association(remote: &RemoteShared, target: &Address) {
  remote
    .handle_event(RemoteEvent::OutboundEnqueued {
      authority: TransportEndpoint::new(target.to_string()),
      envelope:  Box::new(test_user_envelope(target)),
      now_ms:    1,
    })
    .expect("outbound event should start association");
  remote
    .handle_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(target.to_string()),
      frame:     WireFrame::Handshake(HandshakePdu::Rsp(HandshakeRsp::new(UniqueAddress::new(target.clone(), 2)))),
      now_ms:    2,
    })
    .expect("handshake response should activate association");
}

fn test_user_envelope(target: &Address) -> OutboundEnvelope {
  let recipient = ActorPathParser::parse(&format!(
    "fraktor.tcp://{}@{}:{}/user/target",
    target.system(),
    target.host(),
    target.port()
  ))
  .expect("recipient path");
  OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new(target.system(), target.host(), Some(target.port()), 2),
    CorrelationId::nil(),
  )
}

fn test_deathwatch_envelope(target: &Address) -> OutboundEnvelope {
  let recipient = ActorPathParser::parse(&format!(
    "fraktor.tcp://{}@{}:{}/user/watcher",
    target.system(),
    target.host(),
    target.port()
  ))
  .expect("recipient path");
  let sender = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/terminated").expect("sender path");
  OutboundEnvelope::new(
    recipient,
    Some(sender),
    AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(10, 0))),
    OutboundPriority::System,
    RemoteNodeId::new(target.system(), target.host(), Some(target.port()), 2),
    CorrelationId::nil(),
  )
}

fn assert_deathwatch_notification_enqueued(receiver: &mut Receiver<RemoteEvent>) {
  assert!(matches!(
    receiver.try_recv(),
    Ok(RemoteEvent::OutboundEnqueued { envelope, .. })
      if envelope.priority() == OutboundPriority::System
        && envelope.message().downcast_ref::<SystemMessage>()
          == Some(&SystemMessage::DeathWatchNotification(Pid::new(10, 0)))
  ));
}

async fn shutdown_with_timeout(remote: RemoteShared, config: RemoteConfig) -> Result<(), RemotingError> {
  let (event_sender, _event_receiver) = mpsc::channel(8);
  timeout(
    Duration::from_secs(1),
    shutdown_remote_and_join(
      remote,
      Some(event_sender),
      Arc::new(Mutex::new(RemotingRunState::new())),
      config,
      Instant::now(),
      StdFlushGate::default(),
    ),
  )
  .await
  .expect("shutdown should not hang")
}

#[test]
fn test_remote_transport_reports_lifecycle_edges() {
  let local = local_address();
  let target = remote_address();
  let mut transport = TestRemoteTransport::new(vec![local.clone()]);

  assert_eq!(transport.shutdown().expect_err("shutdown before start should fail"), TransportError::NotStarted);
  assert_eq!(
    transport.connect_peer(&target).expect_err("connect before start should fail"),
    TransportError::NotStarted
  );
  let (error, _envelope) =
    transport.send(test_user_envelope(&target)).expect_err("send before start should return envelope");
  assert_eq!(error, TransportError::NotStarted);
  assert_eq!(
    transport
      .send_control(&target, ControlPdu::Shutdown { authority: local.to_string() })
      .expect_err("control before start should fail"),
    TransportError::NotStarted
  );
  assert_eq!(
    transport
      .send_flush_request(&target, ControlPdu::Shutdown { authority: local.to_string() }, 0)
      .expect_err("flush before start should fail"),
    TransportError::NotStarted
  );
  assert_eq!(
    transport.send_ack(&target, AckPdu::new(1, 1, 0)).expect_err("ack before start should fail"),
    TransportError::NotStarted
  );
  assert_eq!(
    transport
      .send_handshake(&target, HandshakePdu::Rsp(HandshakeRsp::new(UniqueAddress::new(local.clone(), 1))))
      .expect_err("handshake before start should fail"),
    TransportError::NotStarted
  );
  assert_eq!(
    transport
      .schedule_handshake_timeout(&TransportEndpoint::new(target.to_string()), Duration::from_millis(1), 1)
      .expect_err("timer before start should fail"),
    TransportError::NotStarted
  );
  assert_eq!(
    transport
      .quarantine(&target, Some(1), QuarantineReason::new("test"))
      .expect_err("quarantine before start should fail"),
    TransportError::NotStarted
  );

  transport.start().expect("first start should succeed");
  assert_eq!(transport.start().expect_err("second start should fail"), TransportError::AlreadyRunning);
  transport.connect_peer(&target).expect("running transport should connect");
  transport
    .send_control(&target, ControlPdu::Shutdown { authority: local.to_string() })
    .expect("running transport should send control");
  transport
    .send_flush_request(&target, ControlPdu::Shutdown { authority: local.to_string() }, 0)
    .expect("running transport should send flush request");
  transport.send_ack(&target, AckPdu::new(1, 1, 0)).expect("running transport should send ack");
  transport
    .send_handshake(&target, HandshakePdu::Rsp(HandshakeRsp::new(UniqueAddress::new(local, 1))))
    .expect("running transport should send handshake");
  transport
    .schedule_handshake_timeout(&TransportEndpoint::new(target.to_string()), Duration::from_millis(1), 1)
    .expect("running transport should schedule timer");
  transport.quarantine(&target, Some(1), QuarantineReason::new("test")).expect("running transport should quarantine");
  transport.send(test_user_envelope(&target)).expect("running transport should send envelope");

  transport.send_result = Err(TransportError::Backpressure);
  let (error, _envelope) =
    transport.send(test_user_envelope(&target)).expect_err("configured send error should return envelope");
  assert_eq!(error, TransportError::Backpressure);
  transport.shutdown().expect("shutdown after start should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn shutdown_remote_and_join_completes_without_active_association() {
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(5));
  let remote = remote_shared(config.clone(), TestRemoteTransport::new(vec![local_address()]));

  shutdown_with_timeout(remote, config).await.expect("shutdown without active associations should complete");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn shutdown_remote_and_join_completes_after_flush_timeout() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(10));
  let remote = remote_shared(config.clone(), TestRemoteTransport::new(vec![local]));
  activate_association(&remote, &target);

  shutdown_with_timeout(remote, config).await.expect("shutdown should continue after flush timeout");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn shutdown_remote_and_join_completes_after_flush_start_failure() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote = remote_shared(
    config.clone(),
    TestRemoteTransport::with_flush_result(vec![local], Err(TransportError::Backpressure)),
  );
  activate_association(&remote, &target);

  shutdown_with_timeout(remote, config).await.expect("shutdown should continue after flush start failure");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn shutdown_remote_and_join_continues_when_shutdown_flush_is_not_started() {
  let local = local_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote = remote_shared_not_started(config.clone(), TestRemoteTransport::new(vec![local]));
  let (event_sender, _event_receiver) = mpsc::channel(8);

  timeout(
    Duration::from_secs(1),
    shutdown_remote_and_join(
      remote,
      Some(event_sender),
      Arc::new(Mutex::new(RemotingRunState::new())),
      config,
      Instant::now(),
      StdFlushGate::default(),
    ),
  )
  .await
  .expect("shutdown should not hang")
  .expect("shutdown should continue after flush setup failure");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_completion_releases_notification() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote = remote_shared(config, TestRemoteTransport::new(vec![local]));
  let gate = StdFlushGate::default();
  let (event_sender, mut event_receiver) = mpsc::channel(8);
  activate_association(&remote, &target);

  assert!(gate.submit_notification(&remote, notification(&event_sender, &target, 3)));
  remote
    .handle_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(target.to_string()),
      frame:     WireFrame::Control(ControlPdu::FlushAck {
        authority:     target.to_string(),
        flush_id:      1,
        lane_id:       0,
        expected_acks: 1,
      }),
      now_ms:    4,
    })
    .expect("flush ack should be accepted");
  gate.observe_outcomes(remote.drain_flush_outcomes(), &event_sender);

  assert_deathwatch_notification_enqueued(&mut event_receiver);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_empty_lane_completion_releases_notification() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote = remote_shared(config, TestRemoteTransport::new(vec![local]));
  let gate = StdFlushGate::default();
  let (event_sender, mut event_receiver) = mpsc::channel(8);
  activate_association(&remote, &target);

  assert!(gate.submit_notification(&remote, notification_with_lane_ids(&event_sender, &target, 3, &[])));

  assert_deathwatch_notification_enqueued(&mut event_receiver);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_timer_delivery_observes_closed_event_queue() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::ZERO);
  let remote = remote_shared(config, TestRemoteTransport::new(vec![local]));
  let gate = StdFlushGate::default();
  let (event_sender, event_receiver) = mpsc::channel(8);
  activate_association(&remote, &target);
  drop(event_receiver);

  assert!(gate.submit_notification(&remote, notification(&event_sender, &target, 3)));
  sleep(Duration::from_millis(1)).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_timeout_releases_notification() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote = remote_shared(config, TestRemoteTransport::new(vec![local]));
  let gate = StdFlushGate::default();
  let (event_sender, mut event_receiver) = mpsc::channel(8);
  activate_association(&remote, &target);

  assert!(gate.submit_notification(&remote, notification(&event_sender, &target, 3)));
  remote
    .handle_event(RemoteEvent::FlushTimerFired {
      authority: TransportEndpoint::new(target.to_string()),
      flush_id:  1,
      now_ms:    60,
    })
    .expect("flush timer should be accepted");
  gate.observe_outcomes(remote.drain_flush_outcomes(), &event_sender);

  assert_deathwatch_notification_enqueued(&mut event_receiver);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_start_failure_releases_notification() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote =
    remote_shared(config, TestRemoteTransport::with_flush_result(vec![local], Err(TransportError::Backpressure)));
  let gate = StdFlushGate::default();
  let (event_sender, mut event_receiver) = mpsc::channel(8);
  activate_association(&remote, &target);

  assert!(gate.submit_notification(&remote, notification(&event_sender, &target, 3)));

  assert_deathwatch_notification_enqueued(&mut event_receiver);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_without_active_association_releases_notification() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote = remote_shared(config, TestRemoteTransport::new(vec![local]));
  let gate = StdFlushGate::default();
  let (event_sender, mut event_receiver) = mpsc::channel(8);

  assert!(gate.submit_notification(&remote, notification(&event_sender, &target, 3)));

  assert_deathwatch_notification_enqueued(&mut event_receiver);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_not_started_releases_notification() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_millis(50));
  let remote = remote_shared_not_started(config, TestRemoteTransport::new(vec![local]));
  let gate = StdFlushGate::default();
  let (event_sender, mut event_receiver) = mpsc::channel(8);

  assert!(gate.submit_notification(&remote, notification(&event_sender, &target, 3)));

  assert_deathwatch_notification_enqueued(&mut event_receiver);
}

fn notification<'a>(event_sender: &'a Sender<RemoteEvent>, target: &Address, now_ms: u64) -> StdFlushNotification<'a> {
  notification_with_lane_ids(event_sender, target, now_ms, &[0])
}

fn notification_with_lane_ids<'a>(
  event_sender: &'a Sender<RemoteEvent>,
  target: &Address,
  now_ms: u64,
  lane_ids: &'a [u32],
) -> StdFlushNotification<'a> {
  StdFlushNotification {
    event_sender,
    monotonic_epoch: Instant::now(),
    lane_ids,
    authority: TransportEndpoint::new(target.to_string()),
    envelope: test_deathwatch_envelope(target),
    now_ms,
  }
}
