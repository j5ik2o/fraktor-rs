//! In-memory routing layer that delivers loopback envelopes without a physical transport.

use alloc::{
  boxed::Box,
  format,
  string::{String, ToString},
};

use ahash::RandomState;
use fraktor_actor_rs::core::kernel::{
  event::logging::LogLevel,
  system::{ActorSystem, ActorSystemWeak},
};
use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess};
use hashbrown::HashMap;
use spin::Mutex;

use crate::core::{
  endpoint_reader::EndpointReader,
  endpoint_writer::{EndpointWriterError, EndpointWriterShared},
  envelope::{OutboundMessage, RemotingEnvelope},
  remote_node_id::RemoteNodeId,
};

#[allow(dead_code)]
const LOOPBACK_SCHEME: &str = "fraktor.loopback";

pub(crate) enum LoopbackDeliveryOutcome {
  Delivered,
  Pending(Box<OutboundMessage>),
}

trait LoopbackDeliverer: Send + Sync {
  fn deliver(&self, envelope: RemotingEnvelope);
}

/// Internal deliverer implementation that uses a weak reference to the actor system
/// to avoid circular references and memory leaks in the static registry.
struct LoopbackDelivererImpl {
  reader: EndpointReader,
  system: ActorSystemWeak,
}

impl LoopbackDelivererImpl {
  fn new(reader: EndpointReader, system: ActorSystemWeak) -> Self {
    Self { reader, system }
  }
}

impl LoopbackDeliverer for LoopbackDelivererImpl {
  fn deliver(&self, envelope: RemotingEnvelope) {
    let Some(system) = self.system.upgrade() else {
      // System has been dropped, silently ignore
      return;
    };
    match self.reader.decode(envelope) {
      | Ok(inbound) => {
        if let Err(error) = self.reader.deliver(inbound) {
          system.emit_log(LogLevel::Warn, format!("loopback delivery failed: {error:?}"), None);
        }
      },
      | Err(error) => {
        system.emit_log(LogLevel::Warn, format!("loopback decode failed: {error:?}"), None);
      },
    }
  }
}

fn format_authority(host: &str, port: Option<u16>) -> String {
  match port {
    | Some(port) => format!("{host}:{port}"),
    | None => host.to_string(),
  }
}

type ArcDeliverer = ArcShared<dyn LoopbackDeliverer>;

static REGISTRY: Mutex<Option<HashMap<String, ArcDeliverer, RandomState>>> = Mutex::new(None);

#[allow(dead_code)]
pub(crate) fn scheme() -> &'static str {
  LOOPBACK_SCHEME
}

/// Registers a loopback endpoint for the given authority.
///
/// The system reference is stored as a weak reference to avoid circular references
/// and ensure proper cleanup when the actor system is dropped.
pub(crate) fn register_endpoint(authority: String, reader: EndpointReader, system: ActorSystem) {
  let deliverer: ArcDeliverer = ArcShared::new(LoopbackDelivererImpl::new(reader, system.downgrade()));
  let mut guard = REGISTRY.lock();
  guard.get_or_insert_with(|| HashMap::with_hasher(RandomState::new())).insert(authority, deliverer);
}

/// Unregisters a loopback endpoint for the given authority.
///
/// This should be called during actor system shutdown to release resources
/// held in the static registry.
#[allow(dead_code)]
pub(crate) fn unregister_endpoint(authority: &str) {
  let mut guard = REGISTRY.lock();
  if let Some(map) = guard.as_mut() {
    map.remove(authority);
  }
}

pub(crate) fn try_deliver(
  remote: &RemoteNodeId,
  writer: &EndpointWriterShared,
  message: OutboundMessage,
) -> Result<LoopbackDeliveryOutcome, EndpointWriterError> {
  let authority = format_authority(remote.host(), remote.port());
  let deliverer = REGISTRY.lock().as_ref().and_then(|map| map.get(&authority).cloned());
  let Some(deliverer) = deliverer else {
    return Ok(LoopbackDeliveryOutcome::Pending(Box::new(message)));
  };
  let envelope = writer.with_read(|w| w.serialize_for_loopback(message))?;
  deliverer.deliver(envelope);
  Ok(LoopbackDeliveryOutcome::Delivered)
}
