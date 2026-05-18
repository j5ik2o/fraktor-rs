//! Compression table kind identifiers.

/// Identifies the metadata table used by a compression entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionTableKind {
  /// Actor reference path compression table.
  ActorRef,
  /// Serializer manifest compression table.
  Manifest,
}

impl CompressionTableKind {
  /// Returns the wire identifier for this table kind.
  #[must_use]
  pub const fn to_wire(self) -> u8 {
    match self {
      | Self::ActorRef => 0x00,
      | Self::Manifest => 0x01,
    }
  }

  /// Decodes a compression table kind from a wire identifier.
  #[must_use]
  pub const fn from_wire(value: u8) -> Option<Self> {
    match value {
      | 0x00 => Some(Self::ActorRef),
      | 0x01 => Some(Self::Manifest),
      | _ => None,
    }
  }
}
