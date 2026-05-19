//! Flush scope carried by wire-level flush control messages.

/// Scope of a wire-level flush handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlushScope {
  /// Flush issued as part of remote shutdown.
  Shutdown,
  /// Flush issued before emitting a remote death watch notification.
  BeforeDeathWatchNotification,
}

impl FlushScope {
  /// Converts this scope to its wire discriminator.
  #[must_use]
  pub const fn to_wire(self) -> u8 {
    match self {
      | Self::Shutdown => 0,
      | Self::BeforeDeathWatchNotification => 1,
    }
  }

  /// Converts a wire discriminator into a [`FlushScope`].
  #[must_use]
  pub const fn from_wire(value: u8) -> Option<Self> {
    match value {
      | 0 => Some(Self::Shutdown),
      | 1 => Some(Self::BeforeDeathWatchNotification),
      | _ => None,
    }
  }
}
