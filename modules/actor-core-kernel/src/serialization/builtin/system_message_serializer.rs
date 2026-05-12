//! Built-in serializer for actor system messages.

#[cfg(test)]
#[path = "system_message_serializer_test.rs"]
mod tests;

use alloc::{boxed::Box, vec, vec::Vec};
use core::any::{Any, TypeId};

use crate::{
  actor::{Pid, messaging::system_message::SystemMessage},
  serialization::{error::SerializationError, serializer::Serializer, serializer_id::SerializerId},
};

const STOP_TAG: u8 = 1;
const WATCH_TAG: u8 = 2;
const UNWATCH_TAG: u8 = 3;
const DEATH_WATCH_NOTIFICATION_TAG: u8 = 4;
const TAG_LEN: usize = 1;
const PID_LEN: usize = 12;
const PID_FRAME_LEN: usize = TAG_LEN + PID_LEN;

/// Serializes the DeathWatch subset of runtime system messages.
pub struct SystemMessageSerializer {
  id: SerializerId,
}

impl SystemMessageSerializer {
  /// Creates a new instance with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for SystemMessageSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let system_message = message.downcast_ref::<SystemMessage>().ok_or(SerializationError::InvalidFormat)?;
    match system_message {
      | SystemMessage::Stop => Ok(vec![STOP_TAG]),
      | SystemMessage::Watch(pid) => Ok(encode_pid_message(WATCH_TAG, *pid)),
      | SystemMessage::Unwatch(pid) => Ok(encode_pid_message(UNWATCH_TAG, *pid)),
      | SystemMessage::DeathWatchNotification(pid) => Ok(encode_pid_message(DEATH_WATCH_NOTIFICATION_TAG, *pid)),
      | _ => Err(SerializationError::InvalidFormat),
    }
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let tag = *bytes.first().ok_or(SerializationError::InvalidFormat)?;
    let message = match tag {
      | STOP_TAG => decode_stop(bytes)?,
      | WATCH_TAG => SystemMessage::Watch(decode_pid(bytes)?),
      | UNWATCH_TAG => SystemMessage::Unwatch(decode_pid(bytes)?),
      | DEATH_WATCH_NOTIFICATION_TAG => SystemMessage::DeathWatchNotification(decode_pid(bytes)?),
      | _ => return Err(SerializationError::InvalidFormat),
    };
    Ok(Box::new(message))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn encode_pid_message(tag: u8, pid: Pid) -> Vec<u8> {
  let mut buffer = Vec::with_capacity(PID_FRAME_LEN);
  buffer.push(tag);
  buffer.extend_from_slice(&pid.value().to_le_bytes());
  buffer.extend_from_slice(&pid.generation().to_le_bytes());
  buffer
}

const fn decode_stop(bytes: &[u8]) -> Result<SystemMessage, SerializationError> {
  if bytes.len() != TAG_LEN {
    return Err(SerializationError::InvalidFormat);
  }
  Ok(SystemMessage::Stop)
}

fn decode_pid(bytes: &[u8]) -> Result<Pid, SerializationError> {
  if bytes.len() != PID_FRAME_LEN {
    return Err(SerializationError::InvalidFormat);
  }
  let value = u64::from_le_bytes(bytes[1..9].try_into().map_err(|_| SerializationError::InvalidFormat)?);
  let generation = u32::from_le_bytes(bytes[9..13].try_into().map_err(|_| SerializationError::InvalidFormat)?);
  Ok(Pid::new(value, generation))
}
