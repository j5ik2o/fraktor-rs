//! Acked-delivery frames exchanged on the system-message sub-channel.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::convert::TryInto;

use fraktor_actor_rs::core::event::stream::CorrelationId;

use super::system_message_envelope::{SYSTEM_MESSAGE_FRAME_KIND, SystemMessageEnvelope};
use crate::core::wire_error::WireError;

const VERSION: u8 = 1;
/// Wire kind used for ACK frames.
pub const ACKED_DELIVERY_ACK_FRAME_KIND: u8 = 0x12;
/// Wire kind used for NACK frames.
pub const ACKED_DELIVERY_NACK_FRAME_KIND: u8 = 0x13;

/// System-message sub-channel payloads supporting acked delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AckedDelivery {
  /// Sequenced system message payload.
  SystemMessage(Box<SystemMessageEnvelope>),
  /// Positive acknowledgement for `sequence_no`.
  Ack {
    /// Highest contiguous sequence observed by the receiver.
    sequence_no: u64,
  },
  /// Negative acknowledgement for `sequence_no`.
  Nack {
    /// Highest contiguous sequence number acknowledged by the receiver.
    sequence_no: u64,
  },
}

impl AckedDelivery {
  /// Creates an ACK message.
  #[must_use]
  pub const fn ack(sequence_no: u64) -> Self {
    Self::Ack { sequence_no }
  }

  /// Creates a NACK message.
  #[must_use]
  pub const fn nack(sequence_no: u64) -> Self {
    Self::Nack { sequence_no }
  }

  /// Returns the sequence number carried by this payload.
  #[must_use]
  pub const fn sequence_no(&self) -> u64 {
    match self {
      | Self::SystemMessage(envelope) => envelope.sequence_no(),
      | Self::Ack { sequence_no } | Self::Nack { sequence_no } => *sequence_no,
    }
  }

  /// Returns `true` when this is an ACK frame.
  #[must_use]
  pub const fn is_ack(&self) -> bool {
    matches!(self, Self::Ack { .. })
  }

  /// Returns `true` when this is a NACK frame.
  #[must_use]
  pub const fn is_nack(&self) -> bool {
    matches!(self, Self::Nack { .. })
  }

  /// Encodes the payload as a wire frame.
  #[must_use]
  pub fn encode_frame(&self) -> Vec<u8> {
    match self {
      | Self::SystemMessage(envelope) => envelope.encode_frame(),
      | Self::Ack { sequence_no } => encode_seq_frame(ACKED_DELIVERY_ACK_FRAME_KIND, *sequence_no),
      | Self::Nack { sequence_no } => encode_seq_frame(ACKED_DELIVERY_NACK_FRAME_KIND, *sequence_no),
    }
  }

  /// Decodes an acked-delivery payload from bytes.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the payload is malformed.
  pub fn decode_frame(bytes: &[u8], correlation_id: CorrelationId) -> Result<Self, WireError> {
    if bytes.len() < 2 || bytes[0] != VERSION {
      return Err(WireError::InvalidFormat);
    }
    match bytes[1] {
      | SYSTEM_MESSAGE_FRAME_KIND => {
        let envelope = SystemMessageEnvelope::decode_frame(bytes, correlation_id)?;
        Ok(Self::SystemMessage(Box::new(envelope)))
      },
      | ACKED_DELIVERY_ACK_FRAME_KIND => Ok(Self::ack(decode_seq_frame(bytes)?)),
      | ACKED_DELIVERY_NACK_FRAME_KIND => Ok(Self::nack(decode_seq_frame(bytes)?)),
      | _ => Err(WireError::InvalidFormat),
    }
  }
}

fn encode_seq_frame(kind: u8, sequence_no: u64) -> Vec<u8> {
  let mut buffer = Vec::from([VERSION, kind]);
  buffer.extend_from_slice(&sequence_no.to_le_bytes());
  buffer
}

fn decode_seq_frame(bytes: &[u8]) -> Result<u64, WireError> {
  if bytes.len() != 10 {
    return Err(WireError::InvalidFormat);
  }
  let seq = u64::from_le_bytes(bytes[2..10].try_into().map_err(|_| WireError::InvalidFormat)?);
  Ok(seq)
}
