//! Describes the priority level assigned to outbound messages.

/// Priority used by [`EndpointWriter`](crate::core::endpoint_writer::EndpointWriter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboundPriority {
  /// Indicates a system message that must bypass user traffic.
  System,
  /// Indicates a user message that can be throttled.
  User,
}

impl OutboundPriority {
  /// Returns `true` when the priority represents a system message.
  #[must_use]
  pub const fn is_system(self) -> bool {
    matches!(self, Self::System)
  }

  /// Encodes the priority into a compact wire representation.
  #[must_use]
  pub const fn to_wire(self) -> u8 {
    match self {
      | Self::System => 1,
      | Self::User => 0,
    }
  }

  /// Restores the priority from a wire representation.
  #[must_use]
  pub const fn from_wire(value: u8) -> Option<Self> {
    match value {
      | 0 => Some(Self::User),
      | 1 => Some(Self::System),
      | _ => None,
    }
  }
}
