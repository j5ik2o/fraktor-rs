//! Built-in serializer for miscellaneous remote messages (subset of Pekko `MiscMessageSerializer`).

#[cfg(test)]
mod tests;

use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};
use core::any::{Any, TypeId, type_name_of_val};

use fraktor_utils_core_rs::core::sync::{ArcShared, WeakShared};

use crate::core::kernel::{
  actor::messaging::{AnyMessage, Identify},
  serialization::{
    delegator::SerializationDelegator, error::SerializationError, serialization_registry::SerializationRegistry,
    serialized_message::SerializedMessage, serializer::Serializer, serializer_id::SerializerId,
    string_manifest_serializer::SerializerWithStringManifest,
  },
};

/// Manifest string identifying the [`Identify`] payload (matches Pekko `MiscMessageSerializer`).
pub(crate) const IDENTIFY_MANIFEST: &str = "ID";

/// Serializes a Pekko-compatible subset of misc remote messages.
///
/// This is a subset of Pekko's `MiscMessageSerializer`. Only [`Identify`] is supported in this
/// revision; `ActorIdentity` requires `ActorRef` path serialization and is tracked separately,
/// and `RemoteRouterConfig` depends on routing-layer types that do not yet exist.
pub struct MiscMessageSerializer {
  id:       SerializerId,
  registry: WeakShared<SerializationRegistry>,
}

impl MiscMessageSerializer {
  /// Creates a new serializer with the provided identifier and registry handle.
  #[must_use]
  pub const fn new(id: SerializerId, registry: WeakShared<SerializationRegistry>) -> Self {
    Self { id, registry }
  }

  fn registry(&self) -> Result<ArcShared<SerializationRegistry>, SerializationError> {
    self.registry.upgrade().ok_or(SerializationError::Uninitialized)
  }

  fn encode_identify(&self, identify: &Identify) -> Result<Vec<u8>, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let payload = identify.correlation_id().payload();
    // 第一候補: registry に登録された binding 名 (= 設定で明示された型名)。
    // フォールバック: ランタイム型名 (`type_name_of_val`) を文字列化する。 trait オブジェクト名と
    // なるが診断上は無情報な "<unbound>" よりは追跡しやすい。診断専用で wire には乗らない。
    let payload_type_name =
      registry.binding_name(payload.type_id()).unwrap_or_else(|| String::from(type_name_of_val(payload)));
    let nested = delegator.serialize(payload, &payload_type_name)?;
    Ok(nested.encode())
  }

  fn decode_identify(&self, bytes: &[u8]) -> Result<Identify, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let nested = SerializedMessage::decode(bytes)?;
    let payload = delegator.deserialize(&nested, None)?;
    // Identify は user メッセージ扱い（control でも NotInfluenceReceiveTimeout でもない）。
    // wire 上に flag を載せていないため、deserialize 側では常に false/false で復元する。
    let message = AnyMessage::from_erased(ArcShared::from_boxed(payload), None, false, false);
    Ok(Identify::new(message))
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
      | _ => Err(SerializationError::InvalidFormat),
    }
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(self.decode_identify(bytes)?))
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
    // manifest() は to_binary が成功したメッセージにしか呼ばれない想定だが、
    // 予期しない型が渡されたら即座に観測できるよう debug ビルドではアサートで落とし、
    // release ではログに残したうえで空マニフェストを返す（呼び出し元の to_binary が
    // InvalidFormat を返すので silent-corruption にはならない）。
    debug_assert!(false, "MiscMessageSerializer::manifest called with unsupported type {:?}", message.type_id());
    tracing::error!(type_id = ?message.type_id(), "MiscMessageSerializer::manifest called with unsupported type");
    Cow::Borrowed("")
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    match manifest {
      | IDENTIFY_MANIFEST => Ok(Box::new(self.decode_identify(bytes)?)),
      // 未対応 manifest は `UnknownManifest` を返すことで `SerializationDelegator::deserialize`
      // の manifest-route fallback (delegator.rs) が次の候補シリアライザーへ continue できる。
      // ここで InvalidFormat を返すと alias 経路が壊れ、将来の ActorIdentity / RemoteRouterConfig
      // 等の追加が manifest_routes 共有時にハードフェイルしてしまう。
      | other => Err(SerializationError::UnknownManifest(String::from(other))),
    }
  }
}
