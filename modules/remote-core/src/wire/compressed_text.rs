//! Compressed text metadata used by envelope wire fields.

#[cfg(test)]
#[path = "compressed_text_test.rs"]
mod tests;

use alloc::string::String;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  primitives::{decode_string, encode_string},
  wire_error::WireError,
};

const LITERAL_TAG: u8 = 0x00;
const TABLE_REF_TAG: u8 = 0x01;

/// Text metadata encoded either as a literal string or a compression table reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompressedText {
  /// Literal text value.
  Literal(String),
  /// Compression table entry id.
  TableRef(u32),
}

impl CompressedText {
  /// Creates literal text metadata.
  #[must_use]
  pub const fn literal(value: String) -> Self {
    Self::Literal(value)
  }

  /// Creates compression table reference metadata.
  #[must_use]
  pub const fn table_ref(entry_id: u32) -> Self {
    Self::TableRef(entry_id)
  }

  /// Returns the literal value when this metadata is literal.
  #[must_use]
  pub const fn as_literal(&self) -> Option<&str> {
    match self {
      | Self::Literal(value) => Some(value.as_str()),
      | Self::TableRef(_) => None,
    }
  }

  /// Returns the table reference id when this metadata is a reference.
  #[must_use]
  pub const fn as_table_ref(&self) -> Option<u32> {
    match self {
      | Self::Literal(_) => None,
      | Self::TableRef(entry_id) => Some(*entry_id),
    }
  }
}

pub(crate) fn encode_compressed_text(value: &CompressedText, buf: &mut BytesMut) -> Result<(), WireError> {
  match value {
    | CompressedText::Literal(text) => {
      buf.put_u8(LITERAL_TAG);
      encode_string(text, buf)
    },
    | CompressedText::TableRef(entry_id) => {
      buf.put_u8(TABLE_REF_TAG);
      buf.put_u32(*entry_id);
      Ok(())
    },
  }
}

pub(crate) fn decode_compressed_text(buf: &mut Bytes) -> Result<CompressedText, WireError> {
  if buf.remaining() < 1 {
    return Err(WireError::Truncated);
  }
  match buf.get_u8() {
    | LITERAL_TAG => Ok(CompressedText::Literal(decode_string(buf)?)),
    | TABLE_REF_TAG => {
      if buf.remaining() < 4 {
        return Err(WireError::Truncated);
      }
      Ok(CompressedText::TableRef(buf.get_u32()))
    },
    | _ => Err(WireError::InvalidFormat),
  }
}

pub(crate) fn encode_option_compressed_text(
  value: Option<&CompressedText>,
  buf: &mut BytesMut,
) -> Result<(), WireError> {
  match value {
    | None => {
      buf.put_u8(0);
      Ok(())
    },
    | Some(value) => {
      buf.put_u8(1);
      encode_compressed_text(value, buf)
    },
  }
}

pub(crate) fn decode_option_compressed_text(buf: &mut Bytes) -> Result<Option<CompressedText>, WireError> {
  if buf.remaining() < 1 {
    return Err(WireError::Truncated);
  }
  match buf.get_u8() {
    | 0 => Ok(None),
    | 1 => Ok(Some(decode_compressed_text(buf)?)),
    | _ => Err(WireError::InvalidFormat),
  }
}
