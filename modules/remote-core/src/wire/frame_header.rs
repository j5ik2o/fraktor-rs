//! Frame header: `length(u32 BE) + version(u8) + kind(u8)`.

/// Initial wire format version.
pub const WIRE_VERSION_1: u8 = 0x01;

/// Wire format version that adds system-envelope redelivery metadata.
pub const WIRE_VERSION_2: u8 = 0x02;

/// Wire format version that adds envelope and control compression metadata.
pub const WIRE_VERSION_3: u8 = 0x03;

/// Wire format version that adds address-terminated deployment failures.
pub const WIRE_VERSION_4: u8 = 0x04;

/// Current wire format version.
pub const WIRE_VERSION: u8 = WIRE_VERSION_4;

/// Offset of the PDU kind byte in an encoded frame.
pub const FRAME_KIND_OFFSET: usize = 5;

/// Kind byte for [`crate::wire::EnvelopePdu`].
pub const KIND_ENVELOPE: u8 = 0x01;

/// Kind byte for the `Req` variant of [`crate::wire::HandshakePdu`].
pub const KIND_HANDSHAKE_REQ: u8 = 0x02;

/// Kind byte for the `Rsp` variant of [`crate::wire::HandshakePdu`].
pub const KIND_HANDSHAKE_RSP: u8 = 0x03;

/// Kind byte for [`crate::wire::ControlPdu`].
pub const KIND_CONTROL: u8 = 0x04;

/// Kind byte for [`crate::wire::AckPdu`].
pub const KIND_ACK: u8 = 0x05;

/// Kind byte for [`crate::wire::RemoteDeploymentPdu`].
pub const KIND_DEPLOYMENT: u8 = 0x06;

/// Wire frame header: length prefix + version byte + kind byte.
///
/// The `length` field is the number of bytes that follow the length field itself
/// (i.e. `1` version byte + `1` kind byte + the PDU body), matching the wire
/// specification exactly. This struct is used as a light-weight DTO by the
/// individual `Codec` implementations when they parse a frame header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
  length:  u32,
  version: u8,
  kind:    u8,
}

impl FrameHeader {
  /// Creates a new [`FrameHeader`].
  #[must_use]
  pub const fn new(length: u32, version: u8, kind: u8) -> Self {
    Self { length, version, kind }
  }

  /// Returns the length field (bytes after the length field itself).
  #[must_use]
  pub const fn length(&self) -> u32 {
    self.length
  }

  /// Returns the wire format version byte.
  #[must_use]
  pub const fn version(&self) -> u8 {
    self.version
  }

  /// Returns the PDU kind byte.
  #[must_use]
  pub const fn kind(&self) -> u8 {
    self.kind
  }
}
