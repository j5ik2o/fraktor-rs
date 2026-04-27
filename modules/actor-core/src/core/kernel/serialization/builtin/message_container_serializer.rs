//! Built-in serializer for actor selection message containers.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  any::{Any, TypeId, type_name_of_val},
  convert::TryInto,
};

use fraktor_utils_core_rs::core::sync::{ArcShared, WeakShared};

use crate::core::kernel::{
  actor::{
    actor_selection::{ActorSelectionMessage, SelectionPathElement},
    messaging::AnyMessage,
  },
  serialization::{
    delegator::SerializationDelegator, error::SerializationError, serialization_registry::SerializationRegistry,
    serialized_message::SerializedMessage, serializer::Serializer, serializer_id::SerializerId,
  },
};

const WILDCARD_FALSE: u8 = 0;
const WILDCARD_TRUE: u8 = 1;
const CHILD_NAME_TAG: u8 = 1;
const CHILD_PATTERN_TAG: u8 = 2;
const PARENT_TAG: u8 = 3;

/// Serializes actor selection messages with nested payload metadata.
pub struct MessageContainerSerializer {
  id:       SerializerId,
  registry: WeakShared<SerializationRegistry>,
}

impl MessageContainerSerializer {
  /// Creates a new serializer with the provided identifier and registry handle.
  #[must_use]
  pub const fn new(id: SerializerId, registry: WeakShared<SerializationRegistry>) -> Self {
    Self { id, registry }
  }

  fn registry(&self) -> Result<ArcShared<SerializationRegistry>, SerializationError> {
    self.registry.upgrade().ok_or(SerializationError::Uninitialized)
  }
}

impl Serializer for MessageContainerSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let selection = message.downcast_ref::<ActorSelectionMessage>().ok_or(SerializationError::InvalidFormat)?;
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let payload = selection.message().payload();
    // 第一候補: registry に登録された binding 名 (= 設定で明示された型名)。
    // フォールバック: ランタイム型名 (`type_name_of_val`) を文字列化する。 trait オブジェクト名と
    // なるが診断上は無情報な "<unbound>" よりは追跡しやすい。診断専用で wire には乗らない。
    let payload_type_name =
      registry.binding_name(payload.type_id()).unwrap_or_else(|| String::from(type_name_of_val(payload)));
    let nested = delegator.serialize(payload, &payload_type_name)?;
    encode_selection(selection, &nested)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let registry = self.registry()?;
    let decoded = decode_selection(bytes)?;
    let delegator = SerializationDelegator::new(&registry);
    let payload = delegator.deserialize(&decoded.nested, None)?;
    // ActorSelectionMessage の payload はユーザメッセージ扱い（control でも NotInfluenceReceiveTimeout
    // でもない）。 wire 上に flag を載せていないため、deserialize 側では常に false/false
    // で復元する。
    let message = AnyMessage::from_erased(ArcShared::from_boxed(payload), None, false, false);
    Ok(Box::new(ActorSelectionMessage::new(message, decoded.elements, decoded.wildcard_fan_out)))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

struct DecodedSelection {
  nested:           SerializedMessage,
  elements:         Vec<SelectionPathElement>,
  wildcard_fan_out: bool,
}

fn encode_selection(
  selection: &ActorSelectionMessage,
  nested: &SerializedMessage,
) -> Result<Vec<u8>, SerializationError> {
  let nested_bytes = nested.encode();
  let mut buffer = Vec::new();
  write_len_prefixed_bytes(&mut buffer, &nested_bytes)?;
  buffer.push(encode_wildcard(selection.wildcard_fan_out()));
  write_u32(&mut buffer, selection.elements().len())?;
  for element in selection.elements() {
    encode_element(&mut buffer, element)?;
  }
  Ok(buffer)
}

fn decode_selection(bytes: &[u8]) -> Result<DecodedSelection, SerializationError> {
  let mut cursor = Cursor::new(bytes);
  let nested_bytes = cursor.read_len_prefixed_bytes()?;
  let nested = SerializedMessage::decode(nested_bytes)?;
  let wildcard_fan_out = decode_wildcard(cursor.read_u8()?)?;
  let element_count = cursor.read_u32()? as usize;
  // 信頼できない wire データから読み出した `element_count` をそのまま `Vec::with_capacity`
  // に渡すと、悪意ある peer が `u32::MAX` 近くの値を仕掛けるだけで multi-GB の確保要求が
  // 発生し、即座にプロセスを panic させられる (cursor bugbot 報告)。
  // 各要素は最低 1 バイト (Parent タグ) を消費するため、残バイト数を上限に capacity を
  // 制限することで「要素数 > 残バイト数」の不整合 payload を予約段階で防ぐ。
  let remaining = cursor.remaining();
  if element_count > remaining {
    return Err(SerializationError::InvalidFormat);
  }
  let mut elements = Vec::with_capacity(element_count);
  for _ in 0..element_count {
    elements.push(decode_element(&mut cursor)?);
  }
  if !cursor.is_finished() {
    return Err(SerializationError::InvalidFormat);
  }
  Ok(DecodedSelection { nested, elements, wildcard_fan_out })
}

fn encode_element(buffer: &mut Vec<u8>, element: &SelectionPathElement) -> Result<(), SerializationError> {
  match element {
    | SelectionPathElement::ChildName(name) => {
      buffer.push(CHILD_NAME_TAG);
      write_len_prefixed_bytes(buffer, name.as_bytes())
    },
    | SelectionPathElement::ChildPattern(pattern) => {
      buffer.push(CHILD_PATTERN_TAG);
      write_len_prefixed_bytes(buffer, pattern.as_bytes())
    },
    | SelectionPathElement::Parent => {
      buffer.push(PARENT_TAG);
      Ok(())
    },
  }
}

fn decode_element(cursor: &mut Cursor<'_>) -> Result<SelectionPathElement, SerializationError> {
  match cursor.read_u8()? {
    | CHILD_NAME_TAG => Ok(SelectionPathElement::ChildName(read_string(cursor)?)),
    | CHILD_PATTERN_TAG => Ok(SelectionPathElement::ChildPattern(read_string(cursor)?)),
    | PARENT_TAG => Ok(SelectionPathElement::Parent),
    | _ => Err(SerializationError::InvalidFormat),
  }
}

const fn encode_wildcard(value: bool) -> u8 {
  if value { WILDCARD_TRUE } else { WILDCARD_FALSE }
}

const fn decode_wildcard(value: u8) -> Result<bool, SerializationError> {
  match value {
    | WILDCARD_FALSE => Ok(false),
    | WILDCARD_TRUE => Ok(true),
    | _ => Err(SerializationError::InvalidFormat),
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

fn read_string(cursor: &mut Cursor<'_>) -> Result<String, SerializationError> {
  let bytes = cursor.read_len_prefixed_bytes()?;
  let value = core::str::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
  Ok(String::from(value))
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

  /// Bytes still available to read.
  const fn remaining(&self) -> usize {
    self.bytes.len().saturating_sub(self.offset)
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
}
