//! Built-in serializer for miscellaneous remote messages (subset of Pekko `MiscMessageSerializer`).

#[cfg(test)]
mod tests;

use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};
use core::{
  any::{Any, TypeId, type_name_of_val},
  convert::TryInto,
};

use fraktor_utils_core_rs::core::sync::{ArcShared, WeakShared};

use crate::core::kernel::{
  actor::{
    Address,
    actor_path::ActorPathParser,
    actor_ref::ActorRef,
    deploy::RemoteScope,
    error::ActorError,
    messaging::{ActorIdentity, AnyMessage, Identify, Status},
  },
  routing::{ConsistentHashingPool, Pool, RandomPool, RemoteRouterConfig, RoundRobinPool, SmallestMailboxPool},
  serialization::{
    delegator::SerializationDelegator, error::SerializationError, not_serializable_error::NotSerializableError,
    serialization_registry::SerializationRegistry, serialized_message::SerializedMessage, serializer::Serializer,
    serializer_id::SerializerId, string_manifest_serializer::SerializerWithStringManifest,
  },
  system::state::SystemStateWeak,
};

/// Manifest string identifying the [`Identify`] payload (matches Pekko `MiscMessageSerializer`).
pub(crate) const IDENTIFY_MANIFEST: &str = "A";
// Pekko 互換のため manifest を `"A"` に合わせたが、 旧 fraktor 実装で `"ID"` を発行していた
// 系から流れてくるメッセージも復号できるよう `from_binary_with_manifest` 側で legacy
// alias を許容する。 encode 経路は新 manifest だけを使う。
const LEGACY_IDENTIFY_MANIFEST: &str = "ID";
/// Manifest string identifying an [`ActorIdentity`] payload (matches Pekko
/// `MiscMessageSerializer`).
pub(crate) const ACTOR_IDENTITY_MANIFEST: &str = "B";
/// Manifest string identifying a [`Status::Success`] payload (matches Pekko
/// `MiscMessageSerializer`).
pub(crate) const STATUS_SUCCESS_MANIFEST: &str = "D";
/// Manifest string identifying a [`Status::Failure`] payload (matches Pekko
/// `MiscMessageSerializer`).
pub(crate) const STATUS_FAILURE_MANIFEST: &str = "E";
/// Manifest string identifying a [`RemoteScope`] payload (matches Pekko `MiscMessageSerializer`).
pub(crate) const REMOTE_SCOPE_MANIFEST: &str = "RS";
/// Manifest string identifying a [`RemoteRouterConfig`] payload (matches Pekko
/// `MiscMessageSerializer`).
pub(crate) const REMOTE_ROUTER_CONFIG_MANIFEST: &str = "RORRC";

const RECOVERABLE_ERROR_TAG: u8 = 1;
const FATAL_ERROR_TAG: u8 = 2;
const ESCALATE_ERROR_TAG: u8 = 3;
const SMALLEST_MAILBOX_POOL_TAG: u8 = 1;
const ROUND_ROBIN_POOL_TAG: u8 = 2;
const RANDOM_POOL_TAG: u8 = 3;
const LEN_PREFIX_BYTES: usize = core::mem::size_of::<u32>();
const ENCODED_ADDRESS_STRING_FIELD_COUNT: usize = 3;
const ENCODED_PORT_BYTES: usize = core::mem::size_of::<u32>();
const MIN_ENCODED_ADDRESS_BYTES: usize = LEN_PREFIX_BYTES * ENCODED_ADDRESS_STRING_FIELD_COUNT + ENCODED_PORT_BYTES;

/// Serializes a Pekko-compatible subset of misc remote messages.
///
/// This is a subset of Pekko's `MiscMessageSerializer`.
pub struct MiscMessageSerializer {
  id:           SerializerId,
  registry:     WeakShared<SerializationRegistry>,
  system_state: Option<SystemStateWeak>,
}

impl MiscMessageSerializer {
  /// Creates a new serializer with the provided identifier and registry handle.
  #[must_use]
  pub const fn new(id: SerializerId, registry: WeakShared<SerializationRegistry>) -> Self {
    Self { id, registry, system_state: None }
  }

  /// Creates a new serializer that can resolve serialized actor refs through the actor system.
  #[must_use]
  pub const fn new_with_system_state(
    id: SerializerId,
    registry: WeakShared<SerializationRegistry>,
    system_state: SystemStateWeak,
  ) -> Self {
    Self { id, registry, system_state: Some(system_state) }
  }

  fn registry(&self) -> Result<ArcShared<SerializationRegistry>, SerializationError> {
    self.registry.upgrade().ok_or(SerializationError::Uninitialized)
  }

  fn encode_identify(&self, identify: &Identify) -> Result<Vec<u8>, SerializationError> {
    self.encode_any_message(identify.correlation_id())
  }

  fn encode_status_success(&self, payload: &AnyMessage) -> Result<Vec<u8>, SerializationError> {
    self.encode_any_message(payload)
  }

  fn encode_actor_identity(&self, identity: &ActorIdentity) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    let correlation_id = self.encode_any_message(identity.correlation_id())?;
    write_len_prefixed_bytes(&mut buffer, &correlation_id)?;
    match identity.actor_ref() {
      | Some(actor_ref) => {
        buffer.push(1);
        let path = Self::serialized_actor_ref_path(actor_ref)?;
        write_len_prefixed_bytes(&mut buffer, path.as_bytes())?;
      },
      | None => buffer.push(0),
    }
    Ok(buffer)
  }

  fn encode_any_message(&self, message: &AnyMessage) -> Result<Vec<u8>, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let payload = message.payload();
    // 第一候補: registry に登録された binding 名 (= 設定で明示された型名)。
    // フォールバック: ランタイム型名 (`type_name_of_val`) を文字列化する。 trait オブジェクト名と
    // なるが診断上は無情報な "<unbound>" よりは追跡しやすい。診断専用で wire には乗らない。
    let payload_type_name =
      registry.binding_name(payload.type_id()).unwrap_or_else(|| String::from(type_name_of_val(payload)));
    let nested = delegator.serialize(payload, &payload_type_name)?;
    Ok(nested.encode())
  }

  fn decode_identify(&self, bytes: &[u8]) -> Result<Identify, SerializationError> {
    Ok(Identify::new(self.decode_any_message(bytes)?))
  }

  fn decode_actor_identity(&self, bytes: &[u8]) -> Result<ActorIdentity, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let correlation_id = self.decode_any_message(cursor.read_len_prefixed_bytes()?)?;
    let actor_ref = match cursor.read_u8()? {
      | 0 => None,
      | 1 => {
        let path = cursor.read_string()?;
        Some(self.deserialize_actor_ref(&path)?)
      },
      | _ => return Err(SerializationError::InvalidFormat),
    };
    if !cursor.is_finished() {
      return Err(SerializationError::InvalidFormat);
    }
    Ok(ActorIdentity::new(correlation_id, actor_ref))
  }

  fn decode_status_success(&self, bytes: &[u8]) -> Result<Status, SerializationError> {
    Ok(Status::Success(self.decode_any_message(bytes)?))
  }

  fn decode_any_message(&self, bytes: &[u8]) -> Result<AnyMessage, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let nested = SerializedMessage::decode(bytes)?;
    let payload = delegator.deserialize(&nested, None)?;
    // misc serializer の payload は user メッセージ扱い（control でも NotInfluenceReceiveTimeout
    // でもない）。 wire 上に flag を載せていないため、deserialize 側では常に false/false
    // で復元する。
    Ok(AnyMessage::from_erased(ArcShared::from_boxed(payload), None, false, false))
  }

  fn encode_remote_scope(scope: &RemoteScope) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    Self::write_address(&mut buffer, scope.node())?;
    Ok(buffer)
  }

  fn decode_remote_scope(bytes: &[u8]) -> Result<RemoteScope, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let address = cursor.read_address()?;
    if !cursor.is_finished() {
      return Err(SerializationError::InvalidFormat);
    }
    Ok(RemoteScope::new(address))
  }

  fn encode_remote_router_config<P: SerializableRemoteRouterPool>(
    config: &RemoteRouterConfig<P>,
  ) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    buffer.push(P::WIRE_TAG);
    write_u32(&mut buffer, config.local().nr_of_instances())?;
    write_len_prefixed_bytes(&mut buffer, config.local().router_dispatcher().as_bytes())?;
    write_u32(&mut buffer, config.nodes().len())?;
    for node in config.nodes() {
      Self::write_address(&mut buffer, node)?;
    }
    Ok(buffer)
  }

  fn decode_remote_router_config(bytes: &[u8]) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let pool_tag = cursor.read_u8()?;
    let nr_of_instances = cursor.read_u32()? as usize;
    if nr_of_instances == 0 {
      return Err(SerializationError::InvalidFormat);
    }
    let dispatcher = cursor.read_string()?;
    match pool_tag {
      | SmallestMailboxPool::WIRE_TAG => {
        let local = SmallestMailboxPool::from_remote_router_wire(nr_of_instances, dispatcher);
        let nodes = Self::decode_remote_router_nodes(&mut cursor)?;
        Ok(Box::new(RemoteRouterConfig::new(local, nodes)))
      },
      | RoundRobinPool::WIRE_TAG => {
        let local = RoundRobinPool::from_remote_router_wire(nr_of_instances, dispatcher);
        let nodes = Self::decode_remote_router_nodes(&mut cursor)?;
        Ok(Box::new(RemoteRouterConfig::new(local, nodes)))
      },
      | RandomPool::WIRE_TAG => {
        let local = RandomPool::from_remote_router_wire(nr_of_instances, dispatcher);
        let nodes = Self::decode_remote_router_nodes(&mut cursor)?;
        Ok(Box::new(RemoteRouterConfig::new(local, nodes)))
      },
      | _ => Err(SerializationError::InvalidFormat),
    }
  }

  fn decode_remote_router_nodes(cursor: &mut Cursor<'_>) -> Result<Vec<Address>, SerializationError> {
    let node_count = cursor.read_u32()? as usize;
    if node_count == 0 {
      return Err(SerializationError::InvalidFormat);
    }
    let max_nodes = cursor.remaining() / MIN_ENCODED_ADDRESS_BYTES;
    if node_count > max_nodes {
      return Err(SerializationError::InvalidFormat);
    }
    let mut nodes = Vec::with_capacity(node_count);
    for _ in 0..node_count {
      nodes.push(cursor.read_address()?);
    }
    if !cursor.is_finished() {
      return Err(SerializationError::InvalidFormat);
    }
    Ok(nodes)
  }

  fn write_address(buffer: &mut Vec<u8>, address: &Address) -> Result<(), SerializationError> {
    let host = address.host().ok_or(SerializationError::InvalidFormat)?;
    let port = address.port().ok_or(SerializationError::InvalidFormat)?;
    write_len_prefixed_bytes(buffer, address.protocol().as_bytes())?;
    write_len_prefixed_bytes(buffer, address.system().as_bytes())?;
    write_len_prefixed_bytes(buffer, host.as_bytes())?;
    // Pekko の MiscMessageSerializer は protobuf int32 で port を符号化する。 u16 だと
    // 将来 protobuf 互換に切り替えるときに silent な mis-parse になるため、 4 バイト幅で
    // 符号化しておく。
    buffer.extend_from_slice(&u32::from(port).to_le_bytes());
    Ok(())
  }

  fn encode_status_failure(error: &ActorError) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    let tag = match error {
      | ActorError::Recoverable(_) => RECOVERABLE_ERROR_TAG,
      | ActorError::Fatal(_) => FATAL_ERROR_TAG,
      | ActorError::Escalate(_) => ESCALATE_ERROR_TAG,
    };
    buffer.push(tag);
    write_len_prefixed_bytes(&mut buffer, error.reason().as_str().as_bytes())?;
    Ok(buffer)
  }

  fn decode_status_failure(bytes: &[u8]) -> Result<Status, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let tag = cursor.read_u8()?;
    let reason = cursor.read_string()?;
    if !cursor.is_finished() {
      return Err(SerializationError::InvalidFormat);
    }
    let error = match tag {
      | RECOVERABLE_ERROR_TAG => ActorError::recoverable(reason),
      | FATAL_ERROR_TAG => ActorError::fatal(reason),
      | ESCALATE_ERROR_TAG => ActorError::escalate(reason),
      | _ => return Err(SerializationError::InvalidFormat),
    };
    Ok(Status::Failure(error))
  }

  fn serialized_actor_ref_path(actor_ref: &ActorRef) -> Result<String, SerializationError> {
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).ok_or_else(Self::actor_ref_not_serializable)
  }

  // Phase 2 では `ActorIdentity::found` で運ばれた path をローカル `ActorPathRegistry`
  // でしか解決しない。 送信側 system の authority を持つ remote path は本ローカル lookup
  // ではヒットしないため `actor_ref_not_serializable` を返す。 cross-system での復元は remote
  // `ActorRef` 構築が 整う Phase 3 hard 側で `RemoteActorRefProvider`
  // 経由のブランチを追加して扱う。
  fn deserialize_actor_ref(&self, path: &str) -> Result<ActorRef, SerializationError> {
    let path = ActorPathParser::parse(path).map_err(|_| SerializationError::InvalidFormat)?;
    let Some(system_state) = self.system_state.as_ref().and_then(SystemStateWeak::upgrade) else {
      return Err(Self::actor_ref_not_serializable());
    };
    // pid 解決と cell 取得の 2 段ルックアップ間で actor がライフサイクル終了する race も
    // ありうるため、両方の None を `actor_ref_not_serializable` に集約する。
    system_state
      .with_actor_path_registry(|registry| registry.pid_for(&path))
      .and_then(|pid| system_state.cell(&pid))
      .map(|cell| cell.actor_ref())
      .ok_or_else(Self::actor_ref_not_serializable)
  }

  fn actor_ref_not_serializable() -> SerializationError {
    SerializationError::NotSerializable(NotSerializableError::new("ActorRef", None, None, None, None))
  }

  fn remote_router_config_not_serializable(type_name: &'static str, serializer_id: SerializerId) -> SerializationError {
    SerializationError::NotSerializable(NotSerializableError::new(type_name, Some(serializer_id), None, None, None))
  }

  fn decode_remote_router_config_with_type_hint<P: SerializableRemoteRouterPool + Send + Sync + 'static>(
    bytes: &[u8],
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let decoded = Self::decode_remote_router_config(bytes)?;
    if decoded.is::<RemoteRouterConfig<P>>() { Ok(decoded) } else { Err(SerializationError::InvalidFormat) }
  }
}

trait SerializableRemoteRouterPool: Pool + Sized {
  const WIRE_TAG: u8;

  fn from_remote_router_wire(nr_of_instances: usize, router_dispatcher: String) -> Self;
}

impl SerializableRemoteRouterPool for SmallestMailboxPool {
  const WIRE_TAG: u8 = SMALLEST_MAILBOX_POOL_TAG;

  fn from_remote_router_wire(nr_of_instances: usize, router_dispatcher: String) -> Self {
    Self::new(nr_of_instances).with_dispatcher(router_dispatcher)
  }
}

impl SerializableRemoteRouterPool for RoundRobinPool {
  const WIRE_TAG: u8 = ROUND_ROBIN_POOL_TAG;

  fn from_remote_router_wire(nr_of_instances: usize, router_dispatcher: String) -> Self {
    Self::new(nr_of_instances).with_dispatcher(router_dispatcher)
  }
}

impl SerializableRemoteRouterPool for RandomPool {
  const WIRE_TAG: u8 = RANDOM_POOL_TAG;

  fn from_remote_router_wire(nr_of_instances: usize, router_dispatcher: String) -> Self {
    Self::new(nr_of_instances).with_dispatcher(router_dispatcher)
  }
}

impl Serializer for MiscMessageSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    match message.type_id() {
      | id if id == TypeId::of::<Identify>() => {
        let identify = message.downcast_ref::<Identify>().ok_or(SerializationError::InvalidFormat)?;
        self.encode_identify(identify)
      },
      | id if id == TypeId::of::<ActorIdentity>() => {
        let identity = message.downcast_ref::<ActorIdentity>().ok_or(SerializationError::InvalidFormat)?;
        self.encode_actor_identity(identity)
      },
      | id if id == TypeId::of::<RemoteScope>() => {
        let scope = message.downcast_ref::<RemoteScope>().ok_or(SerializationError::InvalidFormat)?;
        Self::encode_remote_scope(scope)
      },
      | id if id == TypeId::of::<RemoteRouterConfig<SmallestMailboxPool>>() => {
        let config =
          message.downcast_ref::<RemoteRouterConfig<SmallestMailboxPool>>().ok_or(SerializationError::InvalidFormat)?;
        Self::encode_remote_router_config(config)
      },
      | id if id == TypeId::of::<RemoteRouterConfig<RoundRobinPool>>() => {
        let config =
          message.downcast_ref::<RemoteRouterConfig<RoundRobinPool>>().ok_or(SerializationError::InvalidFormat)?;
        Self::encode_remote_router_config(config)
      },
      | id if id == TypeId::of::<RemoteRouterConfig<RandomPool>>() => {
        let config =
          message.downcast_ref::<RemoteRouterConfig<RandomPool>>().ok_or(SerializationError::InvalidFormat)?;
        Self::encode_remote_router_config(config)
      },
      | id if id == TypeId::of::<RemoteRouterConfig<ConsistentHashingPool>>() => {
        Err(Self::remote_router_config_not_serializable("RemoteRouterConfig<ConsistentHashingPool>", self.id))
      },
      | id if id == TypeId::of::<Status>() => {
        let status = message.downcast_ref::<Status>().ok_or(SerializationError::InvalidFormat)?;
        match status {
          | Status::Success(payload) => self.encode_status_success(payload),
          | Status::Failure(error) => Self::encode_status_failure(error),
        }
      },
      | _ => Err(SerializationError::InvalidFormat),
    }
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    // include_manifest() == true なので通常は from_binary_with_manifest が呼ばれるが、
    // include_manifest を確認しない呼び出し元から直接 from_binary に来た場合でも、
    // type_hint が手掛かりとして渡されていれば対応する decoder にディスパッチする。
    // type_hint が無い場合は Pekko の MiscMessageSerializer 由来の単一型 (Identify) として
    // 後方互換に decode する。未知の TypeId は silent な mis-decode を避けて InvalidFormat を返す。
    let Some(type_id) = type_hint else {
      return Ok(Box::new(self.decode_identify(bytes)?));
    };
    if type_id == TypeId::of::<Identify>() {
      return Ok(Box::new(self.decode_identify(bytes)?));
    }
    if type_id == TypeId::of::<ActorIdentity>() {
      return Ok(Box::new(self.decode_actor_identity(bytes)?));
    }
    if type_id == TypeId::of::<RemoteScope>() {
      return Ok(Box::new(Self::decode_remote_scope(bytes)?));
    }
    if type_id == TypeId::of::<RemoteRouterConfig<SmallestMailboxPool>>() {
      return Self::decode_remote_router_config_with_type_hint::<SmallestMailboxPool>(bytes);
    }
    if type_id == TypeId::of::<RemoteRouterConfig<RoundRobinPool>>() {
      return Self::decode_remote_router_config_with_type_hint::<RoundRobinPool>(bytes);
    }
    if type_id == TypeId::of::<RemoteRouterConfig<RandomPool>>() {
      return Self::decode_remote_router_config_with_type_hint::<RandomPool>(bytes);
    }
    // Status は Success / Failure の判別に manifest が必要。 type_hint だけでは
    // どちらの variant か決まらないため、 from_binary_with_manifest 経由を要求する。
    Err(SerializationError::InvalidFormat)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for MiscMessageSerializer {
  fn manifest(&self, message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    if message.downcast_ref::<Identify>().is_some() {
      return Cow::Borrowed(IDENTIFY_MANIFEST);
    }
    if message.downcast_ref::<ActorIdentity>().is_some() {
      return Cow::Borrowed(ACTOR_IDENTITY_MANIFEST);
    }
    if message.downcast_ref::<RemoteScope>().is_some() {
      return Cow::Borrowed(REMOTE_SCOPE_MANIFEST);
    }
    if message.downcast_ref::<RemoteRouterConfig<SmallestMailboxPool>>().is_some() {
      return Cow::Borrowed(REMOTE_ROUTER_CONFIG_MANIFEST);
    }
    if message.downcast_ref::<RemoteRouterConfig<RoundRobinPool>>().is_some() {
      return Cow::Borrowed(REMOTE_ROUTER_CONFIG_MANIFEST);
    }
    if message.downcast_ref::<RemoteRouterConfig<RandomPool>>().is_some() {
      return Cow::Borrowed(REMOTE_ROUTER_CONFIG_MANIFEST);
    }
    if let Some(status) = message.downcast_ref::<Status>() {
      return match status {
        | Status::Success(_) => Cow::Borrowed(STATUS_SUCCESS_MANIFEST),
        | Status::Failure(_) => Cow::Borrowed(STATUS_FAILURE_MANIFEST),
      };
    }
    // `manifest()` cannot return `Result`; normal serialization has already failed in
    // `to_binary`. Keep direct manifest misuse observable without panicking the process.
    let type_id = message.type_id();
    tracing::error!(serializer = "MiscMessageSerializer", ?type_id, "manifest() called with unsupported type");
    Cow::Borrowed("")
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    match manifest {
      | IDENTIFY_MANIFEST | LEGACY_IDENTIFY_MANIFEST => Ok(Box::new(self.decode_identify(bytes)?)),
      | ACTOR_IDENTITY_MANIFEST => Ok(Box::new(self.decode_actor_identity(bytes)?)),
      | REMOTE_SCOPE_MANIFEST => Ok(Box::new(Self::decode_remote_scope(bytes)?)),
      | REMOTE_ROUTER_CONFIG_MANIFEST => Self::decode_remote_router_config(bytes),
      | STATUS_SUCCESS_MANIFEST => Ok(Box::new(self.decode_status_success(bytes)?)),
      | STATUS_FAILURE_MANIFEST => Ok(Box::new(Self::decode_status_failure(bytes)?)),
      // 未対応 manifest は `UnknownManifest` を返すことで `SerializationDelegator::deserialize`
      // の manifest-route fallback (delegator.rs) が次の候補シリアライザーへ continue できる。
      // ここで InvalidFormat を返すと alias 経路が壊れ、将来の ActorIdentity / RemoteRouterConfig
      // 等の追加が manifest_routes 共有時にハードフェイルしてしまう。
      | other => Err(SerializationError::UnknownManifest(String::from(other))),
    }
  }
}

fn write_len_prefixed_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) -> Result<(), SerializationError> {
  write_u32(buffer, bytes.len())?;
  buffer.extend_from_slice(bytes);
  Ok(())
}

fn write_u32(buffer: &mut Vec<u8>, value: usize) -> Result<(), SerializationError> {
  let value = u32::try_from(value).map_err(|_| SerializationError::InvalidFormat)?;
  buffer.extend_from_slice(&value.to_le_bytes());
  Ok(())
}

struct Cursor<'a> {
  bytes:  &'a [u8],
  offset: usize,
}

impl<'a> Cursor<'a> {
  const fn new(bytes: &'a [u8]) -> Self {
    Self { bytes, offset: 0 }
  }

  const fn is_finished(&self) -> bool {
    self.offset == self.bytes.len()
  }

  const fn remaining(&self) -> usize {
    self.bytes.len() - self.offset
  }

  fn read_u8(&mut self) -> Result<u8, SerializationError> {
    let value = *self.bytes.get(self.offset).ok_or(SerializationError::InvalidFormat)?;
    self.offset += 1;
    Ok(value)
  }

  fn read_u32(&mut self) -> Result<u32, SerializationError> {
    let end = self.offset.checked_add(4).ok_or(SerializationError::InvalidFormat)?;
    let bytes = self.bytes.get(self.offset..end).ok_or(SerializationError::InvalidFormat)?;
    self.offset = end;
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| SerializationError::InvalidFormat)?))
  }

  fn read_len_prefixed_bytes(&mut self) -> Result<&'a [u8], SerializationError> {
    let len = self.read_u32()? as usize;
    let end = self.offset.checked_add(len).ok_or(SerializationError::InvalidFormat)?;
    let bytes = self.bytes.get(self.offset..end).ok_or(SerializationError::InvalidFormat)?;
    self.offset = end;
    Ok(bytes)
  }

  fn read_string(&mut self) -> Result<String, SerializationError> {
    let bytes = self.read_len_prefixed_bytes()?;
    let value = core::str::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(String::from(value))
  }

  fn read_address(&mut self) -> Result<Address, SerializationError> {
    let protocol = self.read_string()?;
    let system = self.read_string()?;
    let host = self.read_string()?;
    // Pekko 互換のため port は 4 バイト (u32) 読みだが値域は u16 に収まる必要がある。
    let port_value = self.read_u32()?;
    let port = u16::try_from(port_value).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(Address::new_remote(protocol, system, host, port))
  }
}
