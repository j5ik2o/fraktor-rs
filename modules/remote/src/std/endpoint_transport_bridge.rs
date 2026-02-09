//! Tokio transport bridge that connects EndpointWriter/Reader with transports.

#[cfg(test)]
mod tests;

use alloc::{
  boxed::Box,
  collections::{BTreeMap, VecDeque},
  format,
  string::String,
  sync::Arc,
  vec::Vec,
};
use core::time::Duration;

use fraktor_actor_rs::core::{
  event::{logging::LogLevel, stream::CorrelationId},
  system::ActorSystemWeakGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, SharedAccess},
};
use tokio::{sync::Mutex as TokioMutex, task::JoinHandle, time::sleep};

use crate::core::{
  AssociationState, DeferredEnvelope, EndpointAssociationCommand, EndpointAssociationCoordinatorSharedGeneric,
  EndpointAssociationEffect, EndpointReaderGeneric, EndpointWriterSharedGeneric, EventPublisherGeneric, HandshakeFrame,
  HandshakeKind, InboundFrame, RemoteNodeId, RemoteTransportShared, RemotingEnvelope, TransportBind, TransportChannel,
  TransportEndpoint, TransportError, TransportHandle, TransportInbound, TransportInboundShared, WireError,
};

const OUTBOUND_IDLE_DELAY: Duration = Duration::from_millis(5);

/// Configuration required to bootstrap the transport bridge.
pub struct EndpointTransportBridgeConfig<TB: RuntimeToolbox + 'static> {
  /// Actor system providing scheduling and state access (weak reference).
  pub system:            ActorSystemWeakGeneric<TB>,
  /// Shared endpoint writer feeding outbound frames.
  pub writer:            EndpointWriterSharedGeneric<TB>,
  /// Shared endpoint reader decoding inbound frames.
  pub reader:            ArcShared<EndpointReaderGeneric<TB>>,
  /// Active transport implementation wrapped in a mutex for shared mutable access.
  pub transport:         RemoteTransportShared<TB>,
  /// Event publisher for lifecycle/backpressure events.
  pub event_publisher:   EventPublisherGeneric<TB>,
  /// Canonical host used when binding listeners.
  pub canonical_host:    String,
  /// Canonical port used when binding listeners.
  pub canonical_port:    u16,
  /// Logical system name advertised during handshakes.
  pub system_name:       String,
  /// Timeout used while waiting for a handshake to complete.
  pub handshake_timeout: Duration,
}

/// Handle controlling bridge background tasks.
pub struct EndpointTransportBridgeHandle {
  send_task: JoinHandle<()>,
}

impl EndpointTransportBridgeHandle {
  /// Aborts the background outbound loop.
  pub fn shutdown(self) {
    self.send_task.abort();
  }
}

pub(crate) struct EndpointTransportBridge<TB: RuntimeToolbox + 'static> {
  system:            ActorSystemWeakGeneric<TB>,
  event_publisher:   EventPublisherGeneric<TB>,
  writer:            EndpointWriterSharedGeneric<TB>,
  reader:            ArcShared<EndpointReaderGeneric<TB>>,
  transport:         RemoteTransportShared<TB>,
  host:              String,
  port:              u16,
  system_name:       String,
  handshake_timeout: Duration,
  listener:          TokioMutex<Option<TransportHandle>>,
  channels:          TokioMutex<BTreeMap<String, TransportChannel>>,
  coordinator:       EndpointAssociationCoordinatorSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> EndpointTransportBridge<TB> {
  fn new(config: EndpointTransportBridgeConfig<TB>) -> Arc<Self> {
    Arc::new(Self {
      system:            config.system,
      event_publisher:   config.event_publisher,
      writer:            config.writer,
      reader:            config.reader,
      transport:         config.transport,
      host:              config.canonical_host,
      port:              config.canonical_port,
      system_name:       config.system_name,
      handshake_timeout: config.handshake_timeout,
      listener:          TokioMutex::new(None),
      channels:          TokioMutex::new(BTreeMap::<String, TransportChannel>::new()),
      coordinator:       EndpointAssociationCoordinatorSharedGeneric::new(),
    })
  }

  pub(crate) fn spawn(
    config: EndpointTransportBridgeConfig<TB>,
  ) -> Result<EndpointTransportBridgeHandle, TransportError> {
    let bridge = Self::new(config);
    let bind = TransportBind::new(bridge.host.clone(), Some(bridge.port));
    let handle = bridge.transport.with_write(|t| t.spawn_listener(&bind))?;
    bridge.event_publisher.publish_listen_started(bind.authority(), CorrelationId::from_u128(0));
    *bridge.listener.try_lock().expect("listener mutex uncontended") = Some(handle);
    let handler: TransportInboundShared<TB> =
      TransportInboundShared::new(Box::new(InboundHandler::new(bridge.clone())));
    bridge.transport.with_write(|t| t.install_inbound_handler(handler));
    let send_task = tokio::spawn(Self::drive_outbound(bridge.clone()));
    Ok(EndpointTransportBridgeHandle { send_task })
  }

  async fn drive_outbound(self: Arc<Self>) {
    loop {
      let next = self.writer.with_write(|w| w.try_next());
      match next {
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
    let deferred = DeferredEnvelope::new(envelope);
    let enqueue = self.coordinator.with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.clone(),
        envelope:  alloc::boxed::Box::new(deferred),
      })
    });
    self.process_effects(enqueue.effects).await?;

    match self.coordinator.with_read(|m| m.state(&authority)) {
      | Some(AssociationState::Connected { .. })
      | Some(AssociationState::Associating { .. })
      | Some(AssociationState::Quarantined { .. }) => {},
      | Some(AssociationState::Gated { resume_at }) => {
        if resume_at.is_some_and(|deadline| self.now_millis() >= deadline) {
          let endpoint = TransportEndpoint::new(authority.clone());
          let recover = self.coordinator.with_write(|m| {
            m.handle(EndpointAssociationCommand::Recover {
              authority: authority.clone(),
              endpoint:  Some(endpoint),
              now:       self.now_millis(),
            })
          });
          self.process_effects(recover.effects).await?;
        }
      },
      | Some(AssociationState::Unassociated) | None => {
        let endpoint = TransportEndpoint::new(authority.clone());
        let associate = self.coordinator.with_write(|m| {
          m.handle(EndpointAssociationCommand::Associate {
            authority: authority.clone(),
            endpoint,
            now: self.now_millis(),
          })
        });
        self.process_effects(associate.effects).await?;
      },
    }
    Ok(())
  }

  async fn process_effects(&self, effects: Vec<EndpointAssociationEffect>) -> Result<(), TransportError> {
    let mut queue: VecDeque<EndpointAssociationEffect> = VecDeque::from(effects);
    while let Some(effect) = queue.pop_front() {
      match effect {
        | EndpointAssociationEffect::StartHandshake { authority, endpoint } => {
          let additional = self.handle_start_handshake(&authority, &endpoint).await?;
          queue.extend(additional);
        },
        | EndpointAssociationEffect::DeliverEnvelopes { authority, envelopes } => {
          for envelope in envelopes {
            self.flush_envelope(&authority, envelope).await?;
          }
        },
        | EndpointAssociationEffect::DiscardDeferred { authority, .. } => {
          self.emit_error(format!("discarded deferred envelopes for {authority}"));
        },
        | EndpointAssociationEffect::Lifecycle(event) => self.event_publisher.publish_lifecycle(event),
      }
    }
    Ok(())
  }

  async fn handle_start_handshake(
    &self,
    authority: &str,
    endpoint: &TransportEndpoint,
  ) -> Result<Vec<EndpointAssociationEffect>, TransportError> {
    let channel = if let Some(existing) = self.channels.lock().await.get(authority).copied() {
      existing
    } else {
      let channel = self.transport.with_write(|t| t.open_channel(endpoint))?;
      self.channels.lock().await.insert(authority.to_string(), channel);
      channel
    };
    self.spawn_handshake_timeout_watchdog(authority.to_string());

    let handshake = HandshakeFrame::new(HandshakeKind::Offer, &self.system_name, &self.host, Some(self.port), 0);
    let payload = handshake.encode();
    self.transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil()))?;
    Ok(Vec::new())
  }

  fn spawn_handshake_timeout_watchdog(&self, authority: String) {
    let timeout = self.handshake_timeout;
    let coordinator = self.coordinator.clone();
    let event_publisher = self.event_publisher.clone();
    let system = self.system.clone();
    tokio::spawn(async move {
      sleep(timeout).await;
      let now = system.upgrade().map(|s| s.state().monotonic_now().as_millis() as u64).unwrap_or(0);
      let resume_at = Some(now.saturating_add(Self::duration_millis(timeout)));
      let result = coordinator.with_write(|m| {
        m.handle(EndpointAssociationCommand::HandshakeTimedOut { authority: authority.clone(), resume_at, now })
      });
      for effect in result.effects {
        if let EndpointAssociationEffect::Lifecycle(event) = effect {
          event_publisher.publish_lifecycle(event);
        }
      }
    });
  }

  async fn flush_envelope(&self, authority: &str, deferred: DeferredEnvelope) -> Result<(), TransportError> {
    let envelope = deferred.into_envelope();
    let payload = envelope.encode_frame();
    let channel = self.ensure_channel(authority).await?;
    self.transport.with_write(|t| t.send(&channel, &payload, envelope.correlation_id()))?;
    Ok(())
  }

  async fn ensure_channel(&self, authority: &str) -> Result<TransportChannel, TransportError> {
    if !self.channels.lock().await.contains_key(authority) {
      let endpoint = TransportEndpoint::new(authority.to_string());
      let channel = self.transport.with_write(|t| t.open_channel(&endpoint))?;
      self.channels.lock().await.insert(authority.to_string(), channel);
    }
    let channels = self.channels.lock().await;
    channels.get(authority).copied().ok_or_else(|| TransportError::AuthorityNotBound(authority.to_string()))
  }

  fn emit_error(&self, message: String) {
    if let Some(system) = self.system.upgrade() {
      system.emit_log(LogLevel::Error, message, None);
    }
  }

  fn now_millis(&self) -> u64 {
    self.system.upgrade().map(|s| s.state().monotonic_now().as_millis() as u64).unwrap_or(0)
  }

  fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
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
      if matches!(frame.kind(), HandshakeKind::Offer)
        && let Err(error) = self.send_handshake_ack(&authority).await
      {
        self.emit_error(format!("failed to send handshake ack: {error:?}"));
      }
      let remote =
        RemoteNodeId::new(frame.system_name().to_string(), frame.host().to_string(), frame.port(), frame.uid());
      let accept = self.coordinator.with_write(|m| {
        m.handle(EndpointAssociationCommand::HandshakeAccepted {
          authority,
          remote_node: remote,
          now: self.now_millis(),
        })
      });
      let _ = self.process_effects(accept.effects).await;
    }
    Ok(())
  }

  async fn send_handshake_ack(&self, authority: &str) -> Result<(), TransportError> {
    let channel = self.ensure_channel(authority).await?;
    let ack = HandshakeFrame::new(HandshakeKind::Ack, &self.system_name, &self.host, Some(self.port), 0);
    let payload = ack.encode();
    self.transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil()))
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
  bridge: Arc<EndpointTransportBridge<TB>>,
}

impl<TB: RuntimeToolbox + 'static> InboundHandler<TB> {
  fn new(bridge: Arc<EndpointTransportBridge<TB>>) -> Self {
    Self { bridge }
  }
}

impl<TB: RuntimeToolbox + 'static> TransportInbound for InboundHandler<TB> {
  fn on_frame(&mut self, frame: InboundFrame) {
    let bridge = self.bridge.clone();
    tokio::spawn(async move {
      bridge.handle_inbound_frame(frame).await;
    });
  }
}
