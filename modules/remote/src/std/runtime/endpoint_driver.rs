//! Tokio-based endpoint driver that bridges EndpointWriter/Reader and transports.

use alloc::{
  collections::{BTreeMap, VecDeque},
  format,
  string::String,
  sync::Arc,
  vec::Vec,
};
use core::time::Duration;

use fraktor_actor_rs::core::{event_stream::CorrelationId, logging::LogLevel, system::ActorSystemGeneric};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};
use tokio::{sync::Mutex as TokioMutex, task::JoinHandle, time::sleep};

use crate::core::{
  AssociationState, DeferredEnvelope, EndpointManager, EndpointManagerCommand, EndpointManagerEffect, EndpointReader,
  EndpointWriter, EventPublisher, HandshakeFrame, HandshakeKind, InboundFrame, RemoteNodeId, RemoteTransport,
  RemotingEnvelope, TransportBind, TransportChannel, TransportEndpoint, TransportError, TransportHandle,
  TransportInbound, WireError,
};

const OUTBOUND_IDLE_DELAY: Duration = Duration::from_millis(5);

/// Configuration required to bootstrap the driver.
pub struct EndpointDriverConfig<TB: RuntimeToolbox + 'static> {
  /// Actor system providing scheduling and state access.
  pub system:          ActorSystemGeneric<TB>,
  /// Shared endpoint writer feeding outbound frames.
  pub writer:          ArcShared<EndpointWriter<TB>>,
  /// Shared endpoint reader decoding inbound frames.
  pub reader:          ArcShared<EndpointReader<TB>>,
  /// Active transport implementation.
  pub transport:       ArcShared<dyn RemoteTransport>,
  /// Event publisher for lifecycle/backpressure events.
  pub event_publisher: EventPublisher<TB>,
  /// Canonical host used when binding listeners.
  pub canonical_host:  String,
  /// Canonical port used when binding listeners.
  pub canonical_port:  u16,
  /// Logical system name advertised during handshakes.
  pub system_name:     String,
}

/// Handle controlling driver background tasks.
pub struct EndpointDriverHandle {
  send_task: JoinHandle<()>,
}

impl EndpointDriverHandle {
  /// Aborts the background outbound loop.
  pub fn shutdown(self) {
    self.send_task.abort();
  }
}

pub(crate) struct EndpointDriver<TB: RuntimeToolbox + 'static> {
  system:          ActorSystemGeneric<TB>,
  event_publisher: EventPublisher<TB>,
  writer:          ArcShared<EndpointWriter<TB>>,
  reader:          ArcShared<EndpointReader<TB>>,
  transport:       ArcShared<dyn RemoteTransport>,
  host:            String,
  port:            u16,
  system_name:     String,
  listener:        TokioMutex<Option<TransportHandle>>,
  channels:        TokioMutex<BTreeMap<String, TransportChannel>>,
  peers:           TokioMutex<BTreeMap<String, RemoteNodeId>>,
  manager:         EndpointManager,
}

impl<TB: RuntimeToolbox + 'static> EndpointDriver<TB> {
  fn new(config: EndpointDriverConfig<TB>) -> Arc<Self> {
    Arc::new(Self {
      system:          config.system,
      event_publisher: config.event_publisher,
      writer:          config.writer,
      reader:          config.reader,
      transport:       config.transport,
      host:            config.canonical_host,
      port:            config.canonical_port,
      system_name:     config.system_name,
      listener:        TokioMutex::new(None),
      channels:        TokioMutex::new(BTreeMap::<String, TransportChannel>::new()),
      peers:           TokioMutex::new(BTreeMap::<String, RemoteNodeId>::new()),
      manager:         EndpointManager::new(),
    })
  }

  pub(crate) fn spawn(config: EndpointDriverConfig<TB>) -> Result<EndpointDriverHandle, TransportError> {
    let driver = Self::new(config);
    let bind = TransportBind::new(driver.host.clone(), Some(driver.port));
    let handle = driver.transport.spawn_listener(&bind)?;
    driver.event_publisher.publish_listen_started(bind.authority(), CorrelationId::from_u128(0));
    *driver.listener.try_lock().expect("listener mutex uncontended") = Some(handle);
    driver.transport.install_inbound_handler(ArcShared::new(InboundHandler::new(driver.clone())));
    let send_task = tokio::spawn(Self::drive_outbound(driver.clone()));
    Ok(EndpointDriverHandle { send_task })
  }

  async fn drive_outbound(self: Arc<Self>) {
    loop {
      match self.writer.try_next() {
        | Ok(Some(envelope)) => {
          if let Err(error) = self.handle_outbound_envelope(envelope).await {
            self.emit_error(format!("failed to process outbound envelope: {error:?}"));
          }
        },
        | Ok(None) => sleep(OUTBOUND_IDLE_DELAY).await,
        | Err(error) => {
          self.emit_error(format!("endpoint writer error: {error:?}"));
          sleep(OUTBOUND_IDLE_DELAY).await;
        },
      }
    }
  }

  async fn handle_outbound_envelope(&self, envelope: RemotingEnvelope) -> Result<(), TransportError> {
    let authority = Self::target_authority(envelope.remote_node())
      .ok_or_else(|| TransportError::AuthorityNotBound("missing remote authority".into()))?;
    self.peers.lock().await.insert(authority.clone(), envelope.remote_node().clone());
    let deferred = DeferredEnvelope::new(envelope);
    let enqueue = self.manager.handle(EndpointManagerCommand::EnqueueDeferred {
      authority: authority.clone(),
      envelope:  alloc::boxed::Box::new(deferred),
    });
    self.process_effects(enqueue.effects).await?;

    if !matches!(self.manager.state(&authority), Some(AssociationState::Connected { .. })) {
      let endpoint = TransportEndpoint::new(authority.clone());
      let associate = self.manager.handle(EndpointManagerCommand::Associate {
        authority: authority.clone(),
        endpoint,
        now: self.now_millis(),
      });
      self.process_effects(associate.effects).await?;
    }
    Ok(())
  }

  async fn process_effects(&self, effects: Vec<EndpointManagerEffect>) -> Result<(), TransportError> {
    let mut queue: VecDeque<EndpointManagerEffect> = VecDeque::from(effects);
    while let Some(effect) = queue.pop_front() {
      match effect {
        | EndpointManagerEffect::StartHandshake { authority, endpoint } => {
          let additional = self.handle_start_handshake(&authority, &endpoint).await?;
          queue.extend(additional);
        },
        | EndpointManagerEffect::DeliverEnvelopes { authority, envelopes } => {
          for envelope in envelopes {
            self.flush_envelope(&authority, envelope).await?;
          }
        },
        | EndpointManagerEffect::DiscardDeferred { authority, .. } => {
          self.emit_error(format!("discarded deferred envelopes for {authority}"));
        },
        | EndpointManagerEffect::Lifecycle(event) => self.event_publisher.publish_lifecycle(event),
      }
    }
    Ok(())
  }

  async fn handle_start_handshake(
    &self,
    authority: &str,
    endpoint: &TransportEndpoint,
  ) -> Result<Vec<EndpointManagerEffect>, TransportError> {
    {
      let channels = self.channels.lock().await;
      if channels.contains_key(authority) {
        return Ok(Vec::new());
      }
    }
    let channel = self.transport.open_channel(endpoint)?;
    self.channels.lock().await.insert(authority.to_string(), channel.clone());
    if let Some(remote) = self.peers.lock().await.get(authority).cloned() {
      let handshake = HandshakeFrame::new(HandshakeKind::Offer, &self.system_name, &self.host, Some(self.port), 0);
      let payload = handshake.encode();
      self.transport.send(&channel, &payload, CorrelationId::nil())?;
      let accept = self.manager.handle(EndpointManagerCommand::HandshakeAccepted {
        authority:   authority.to_string(),
        remote_node: remote,
        now:         self.now_millis(),
      });
      return Ok(accept.effects);
    }
    Ok(Vec::new())
  }

  async fn flush_envelope(&self, authority: &str, deferred: DeferredEnvelope) -> Result<(), TransportError> {
    let envelope = deferred.into_envelope();
    let payload = envelope.encode_frame();
    let channel = self.ensure_channel(authority).await?;
    self.transport.send(&channel, &payload, envelope.correlation_id())?;
    Ok(())
  }

  async fn ensure_channel(&self, authority: &str) -> Result<TransportChannel, TransportError> {
    if !self.channels.lock().await.contains_key(authority) {
      let endpoint = TransportEndpoint::new(authority.to_string());
      let channel = self.transport.open_channel(&endpoint)?;
      self.channels.lock().await.insert(authority.to_string(), channel);
    }
    let channels = self.channels.lock().await;
    channels.get(authority).copied().ok_or_else(|| TransportError::AuthorityNotBound(authority.to_string()))
  }

  fn emit_error(&self, message: String) {
    self.system.emit_log(LogLevel::Error, message, None);
  }

  fn now_millis(&self) -> u64 {
    self.system.state().monotonic_now().as_millis() as u64
  }

  fn target_authority(node: &RemoteNodeId) -> Option<String> {
    node.port().map(|port| format!("{}:{port}", node.host()))
  }

  async fn handle_inbound_frame(&self, frame: InboundFrame) {
    if frame.payload().is_empty() {
      return;
    }
    match frame.payload()[1] {
      | 0x01 | 0x02 => {
        if let Err(error) = self.process_handshake_payload(frame.payload().to_vec()).await {
          self.emit_error(format!("failed to decode handshake: {error:?}"));
        }
      },
      | 0x10 => match RemotingEnvelope::decode_frame(frame.payload(), frame.correlation_id()) {
        | Ok(envelope) => self.deliver_inbound(envelope).await,
        | Err(error) => self.emit_error(format!("failed to decode envelope: {error:?}")),
      },
      | _ => {},
    }
  }

  async fn process_handshake_payload(&self, payload: Vec<u8>) -> Result<(), WireError> {
    let frame = HandshakeFrame::decode(&payload)?;
    if let Some(port) = frame.port() {
      let authority = format!("{}:{port}", frame.host());
      let remote =
        RemoteNodeId::new(frame.system_name().to_string(), frame.host().to_string(), frame.port(), frame.uid());
      let accept = self.manager.handle(EndpointManagerCommand::HandshakeAccepted {
        authority,
        remote_node: remote,
        now: self.now_millis(),
      });
      let _ = self.process_effects(accept.effects).await;
    }
    Ok(())
  }

  async fn deliver_inbound(&self, envelope: RemotingEnvelope) {
    match self.reader.decode(envelope) {
      | Ok(inbound) => {
        if let Err(error) = self.reader.deliver(inbound) {
          self.emit_error(format!("failed to deliver inbound envelope: {error:?}"));
        }
      },
      | Err(error) => self.emit_error(format!("failed to deserialize inbound envelope: {error:?}")),
    }
  }
}

struct InboundHandler<TB: RuntimeToolbox + 'static> {
  driver: Arc<EndpointDriver<TB>>,
}

impl<TB: RuntimeToolbox + 'static> InboundHandler<TB> {
  fn new(driver: Arc<EndpointDriver<TB>>) -> Self {
    Self { driver }
  }
}

impl<TB: RuntimeToolbox + 'static> TransportInbound for InboundHandler<TB> {
  fn on_frame(&self, frame: InboundFrame) {
    let driver = self.driver.clone();
    tokio::spawn(async move {
      driver.handle_inbound_frame(frame).await;
    });
  }
}
