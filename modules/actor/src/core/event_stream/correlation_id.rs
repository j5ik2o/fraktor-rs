//! Identifier correlating remoting events and frames.

/// Uniquely tracks remoting operations across transports and observability pipelines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CorrelationId {
  hi: u64,
  lo: u32,
}

impl CorrelationId {
  /// Creates a correlation identifier from explicit components.
  #[must_use]
  pub const fn new(hi: u64, lo: u32) -> Self {
    Self { hi, lo }
  }

  /// Returns the nil correlation identifier.
  #[must_use]
  pub const fn nil() -> Self {
    Self::new(0, 0)
  }

  /// Returns true when the identifier equals the nil value.
  #[must_use]
  pub const fn is_nil(&self) -> bool {
    self.hi == 0 && self.lo == 0
  }

  /// Returns the high 64-bit component.
  #[must_use]
  pub const fn hi(&self) -> u64 {
    self.hi
  }

  /// Returns the low 32-bit component.
  #[must_use]
  pub const fn lo(&self) -> u32 {
    self.lo
  }

  /// Constructs an identifier from a 128-bit value, truncating to 96 bits.
  #[must_use]
  pub fn from_u128(value: u128) -> Self {
    let hi = (value >> 32) as u64;
    let lo = value as u32;
    Self::new(hi, lo)
  }

  /// Returns the identifier encoded as a 96-bit big-endian value.
  #[must_use]
  pub fn to_be_bytes(&self) -> [u8; 12] {
    let mut bytes = [0_u8; 12];
    bytes[..8].copy_from_slice(&self.hi.to_be_bytes());
    bytes[8..].copy_from_slice(&self.lo.to_be_bytes());
    bytes
  }

  /// Converts the identifier into a 128-bit value retaining all 96 bits of precision.
  #[must_use]
  pub fn to_u128(&self) -> u128 {
    ((self.hi as u128) << 32) | (self.lo as u128)
  }
}

impl Default for CorrelationId {
  fn default() -> Self {
    Self::nil()
  }
}
