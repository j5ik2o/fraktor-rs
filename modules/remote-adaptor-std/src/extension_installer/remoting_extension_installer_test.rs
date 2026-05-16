use std::{
  string::String,
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use bytes::Bytes;
use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::ActorPathParser,
    extension::ExtensionInstaller,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  event::stream::CorrelationId,
  serialization::{SerializationExtensionShared, default_serialization_extension_id},
  system::ActorSystemBuildError,
};
use fraktor_remote_core_rs::{
  address::{RemoteNodeId, UniqueAddress},
  association::QuarantineReason,
  envelope::{OutboundEnvelope, OutboundPriority},
  transport::{TransportEndpoint, TransportError},
  wire::{AckPdu, EnvelopePayload, EnvelopePdu, FlushScope, HandshakePdu, HandshakeRsp},
};
use fraktor_utils_core_rs::sync::ArcShared;
use tokio::{
  sync::mpsc::{self, Receiver, Sender},
  time::{sleep, timeout},
};

use super::*;
use crate::{extension_installer::flush_gate::StdFlushNotification, transport::tcp::TcpRemoteTransport};

struct TestRemoteTransport {
  addresses:    Vec<Address>,
  flush_result: Result<(), TransportError>,
}

impl TestRemoteTransport {
  fn new(addresses: Vec<Address>) -> Self {
    Self { addresses, flush_result: Ok(()) }
  }

  fn with_flush_result(addresses: Vec<Address>, flush_result: Result<(), TransportError>) -> Self {
    Self { addresses, flush_result }
  }
}

impl RemoteTransport for TestRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn connect_peer(&mut self, _remote: &Address) -> Result<(), TransportError> {
    Ok(())
  }

  fn send(&mut self, _envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    Ok(())
  }

  fn send_control(&mut self, _remote: &Address, _pdu: ControlPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_flush_request(&mut self, _remote: &Address, _pdu: ControlPdu, _lane_id: u32) -> Result<(), TransportError> {
    self.flush_result.clone()
  }

  fn send_ack(&mut self, _remote: &Address, _pdu: AckPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_handshake(&mut self, _remote: &Address, _pdu: HandshakePdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn schedule_handshake_timeout(
    &mut self,
    _authority: &TransportEndpoint,
    _timeout: Duration,
    _generation: u64,
  ) -> Result<(), TransportError> {
    Ok(())
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
    Ok(())
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

#[test]
fn route_deployment_event_returns_failure_when_daemon_queue_closed() {
  let (sender, receiver) = mpsc::channel(1);
  drop(receiver);
  let dispatcher = DeploymentResponseDispatcher::default();
  let request = RemoteDeploymentCreateRequest::new(
    1,
    2,
    String::from("/user"),
    String::from("child"),
    String::from("echo"),
    String::from("origin@127.0.0.1:2551"),
    1,
    None,
    Bytes::from_static(b"payload"),
  );
  let event = RemoteEvent::InboundFrameReceived {
    authority: TransportEndpoint::new("origin@127.0.0.1:2551"),
    frame:     WireFrame::Deployment(RemoteDeploymentPdu::CreateRequest(request)),
    now_ms:    99,
  };

  let routed = route_deployment_event(event, &sender, &dispatcher).expect("enqueue failure should be routed");

  match routed {
    | RemoteEvent::OutboundDeployment { remote, pdu: RemoteDeploymentPdu::CreateFailure(failure), now_ms } => {
      assert_eq!(remote, Address::new("origin", "127.0.0.1", 2551));
      assert_eq!(failure.correlation_hi(), 1);
      assert_eq!(failure.correlation_lo(), 2);
      assert_eq!(failure.code(), RemoteDeploymentFailureCode::SpawnFailed);
      assert_eq!(now_ms, 99);
    },
    | other => panic!("expected outbound deployment failure, got {other:?}"),
  }
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

fn test_system_envelope_pdu(target: &Address, sequence: u64) -> EnvelopePdu {
  EnvelopePdu::new(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/local"),
    Some(format!("fraktor.tcp://{}@{}:{}/user/sender", target.system(), target.host(), target.port())),
    sequence,
    0,
    OutboundPriority::System.to_wire(),
    EnvelopePayload::new(4, None, Bytes::from_static(b"bad-system-payload")),
  )
  .with_redelivery_sequence(Some(sequence))
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
async fn install_rolls_back_started_remote_when_handle_storage_fails() {
  let installer = RemotingExtensionInstaller::new(
    TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]),
    RemoteConfig::new("127.0.0.1"),
  );
  let (event_sender, _event_receiver) = mpsc::channel(1);
  installer.event_sender.set(event_sender).expect("preloaded event sender should be accepted");
  let system = create_noop_actor_system();

  let error = installer.install(&system).expect_err("handle storage failure should abort install");

  match error {
    | ActorSystemBuildError::Configuration(message) => assert_eq!(message, ALREADY_INSTALLED),
    | other => panic!("expected configuration error, got {other:?}"),
  }
}

#[test]
fn remote_events_use_test_transport_callbacks() {
  let local = local_address();
  let target = remote_address();
  let remote = remote_shared(RemoteConfig::new("127.0.0.1"), TestRemoteTransport::new(vec![local]));
  activate_association(&remote, &target);

  remote
    .handle_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(target.to_string()),
      frame:     WireFrame::Control(ControlPdu::Heartbeat { authority: target.to_string() }),
      now_ms:    3,
    })
    .expect("heartbeat should send a control response");
  remote
    .handle_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(target.to_string()),
      frame:     WireFrame::Envelope(test_system_envelope_pdu(&target, 1)),
      now_ms:    4,
    })
    .expect("system envelope should send an ack");
  remote
    .handle_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(target.to_string()),
      frame:     WireFrame::Control(ControlPdu::FlushRequest {
        authority:     target.to_string(),
        flush_id:      10,
        scope:         FlushScope::Shutdown,
        lane_id:       0,
        expected_acks: 1,
      }),
      now_ms:    5,
    })
    .expect("flush request should send an ack");
  remote
    .handle_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(target.to_string()),
      frame:     WireFrame::Control(ControlPdu::Quarantine {
        authority: target.to_string(),
        reason:    Some(String::from("test")),
      }),
      now_ms:    6,
    })
    .expect("quarantine should reach the transport");
  remote
    .quarantine(&target, Some(2), QuarantineReason::new("test"), 7)
    .expect("local quarantine should reach the transport");
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
  sleep(Duration::from_millis(10)).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn deathwatch_flush_timer_delivery_enqueues_timer_event() {
  let local = local_address();
  let target = remote_address();
  let config = RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::ZERO);
  let remote = remote_shared(config, TestRemoteTransport::new(vec![local]));
  let gate = StdFlushGate::default();
  let (event_sender, mut event_receiver) = mpsc::channel(8);
  activate_association(&remote, &target);

  assert!(gate.submit_notification(&remote, notification(&event_sender, &target, 3)));

  let event = timeout(Duration::from_millis(50), event_receiver.recv())
    .await
    .expect("flush timer event should be delivered")
    .expect("event queue should remain open");
  assert!(matches!(
    event,
    RemoteEvent::FlushTimerFired { authority, flush_id: 1, .. }
      if authority == TransportEndpoint::new(target.to_string())
  ));
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
