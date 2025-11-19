//! In-memory routing layer that delivers loopback envelopes without a physical transport.

use alloc::{
  boxed::Box,
  format,
  string::{String, ToString},
};

use ahash::RandomState;
use fraktor_actor_rs::core::{logging::LogLevel, system::ActorSystemGeneric};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};
use hashbrown::HashMap;
use spin::Mutex;

use crate::core::{
  EndpointWriterError, EndpointWriterGeneric, endpoint_reader::EndpointReaderGeneric,
  outbound_message::OutboundMessage, remote_node_id::RemoteNodeId, remoting_envelope::RemotingEnvelope,
};

#[allow(dead_code)]
const LOOPBACK_SCHEME: &str = "fraktor.loopback";

pub(crate) enum LoopbackDeliveryOutcome<TB: RuntimeToolbox + 'static> {
  Delivered,
  Pending(Box<OutboundMessage<TB>>),
}

trait LoopbackDeliverer: Send + Sync {
  fn deliver(&self, envelope: RemotingEnvelope);
}

struct LoopbackDelivererImpl<TB: RuntimeToolbox + 'static> {
  reader: EndpointReaderGeneric<TB>,
  system: ActorSystemGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> LoopbackDelivererImpl<TB> {
  fn new(reader: EndpointReaderGeneric<TB>, system: ActorSystemGeneric<TB>) -> Self {
    Self { reader, system }
  }
}

impl<TB: RuntimeToolbox + 'static> LoopbackDeliverer for LoopbackDelivererImpl<TB> {
  fn deliver(&self, envelope: RemotingEnvelope) {
    match self.reader.decode(envelope) {
      | Ok(inbound) => {
        if let Err(error) = self.reader.deliver(inbound) {
          self.system.emit_log(LogLevel::Warn, format!("loopback delivery failed: {error:?}"), None);
        }
      },
      | Err(error) => {
        self.system.emit_log(LogLevel::Warn, format!("loopback decode failed: {error:?}"), None);
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

pub(crate) fn register_endpoint<TB>(
  authority: String,
  reader: EndpointReaderGeneric<TB>,
  system: ActorSystemGeneric<TB>,
) where
  TB: RuntimeToolbox + 'static, {
  let deliverer: ArcDeliverer = ArcShared::new(LoopbackDelivererImpl::new(reader, system));
  let mut guard = REGISTRY.lock();
  guard.get_or_insert_with(|| HashMap::with_hasher(RandomState::new())).insert(authority, deliverer);
}

pub(crate) fn try_deliver<TB>(
  remote: &RemoteNodeId,
  writer: &EndpointWriterGeneric<TB>,
  message: OutboundMessage<TB>,
) -> Result<LoopbackDeliveryOutcome<TB>, EndpointWriterError>
where
  TB: RuntimeToolbox + 'static, {
  let authority = format_authority(remote.host(), remote.port());
  let deliverer = REGISTRY.lock().as_ref().and_then(|map| map.get(&authority).cloned());
  let Some(deliverer) = deliverer else {
    return Ok(LoopbackDeliveryOutcome::Pending(Box::new(message)));
  };
  let envelope = writer.serialize_for_loopback(message)?;
  deliverer.deliver(envelope);
  Ok(LoopbackDeliveryOutcome::Delivered)
}
