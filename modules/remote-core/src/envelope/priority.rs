//! Priority level assigned to outbound messages.

/// Priority level attached to an outbound envelope.
///
/// System-priority messages bypass the user traffic queue so that internal
/// signalling (death watch, termination, handshake) cannot be starved behind
/// user payloads. The `to_wire` / `from_wire` helpers align with the wire-format
/// contract: `0 = System`, `1 = User`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutboundPriority {
  /// A system message that must bypass user traffic.
  System,
  /// A user message that can be throttled under backpressure.
  User,
}

impl OutboundPriority {
  /// Returns `true` when the priority represents a system message.
  #[must_use]
  pub const fn is_system(self) -> bool {
    matches!(self, Self::System)
  }

  /// Encodes the priority into its compact wire representation.
  #[must_use]
  pub const fn to_wire(self) -> u8 {
    match self {
      | Self::System => 0,
      | Self::User => 1,
    }
  }

  /// Restores the priority from its wire representation.
  #[must_use]
  pub const fn from_wire(value: u8) -> Option<Self> {
    match value {
      | 0 => Some(Self::System),
      | 1 => Some(Self::User),
      | _ => None,
    }
  }
}
