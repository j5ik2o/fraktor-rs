//! Discriminates remoting handshake frame types.

/// Identifies the type of handshake payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeKind {
  /// Initial offer sent when establishing a connection.
  Offer,
  /// Acknowledgement sent in response to an offer.
  Ack,
}

impl HandshakeKind {
  /// Encodes the kind into the wire discriminator byte.
  #[must_use]
  pub const fn to_wire(self) -> u8 {
    match self {
      | Self::Offer => 0x01,
      | Self::Ack => 0x02,
    }
  }

  /// Restores the handshake kind from the wire discriminator.
  #[must_use]
  pub const fn from_wire(value: u8) -> Option<Self> {
    match value {
      | 0x01 => Some(Self::Offer),
      | 0x02 => Some(Self::Ack),
      | _ => None,
    }
  }
}
