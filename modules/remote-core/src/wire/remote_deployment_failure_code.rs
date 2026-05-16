//! Structured remote deployment failure codes.

/// Wire-safe failure code for remote deployment create failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteDeploymentFailureCode {
  /// The target node does not know the requested factory id.
  UnknownFactoryId,
  /// The requested child name is already in use under the target parent.
  DuplicateChildName,
  /// The deployment payload could not be deserialized.
  DeserializationFailed,
  /// Target-side actor spawn failed.
  SpawnFailed,
  /// The origin-side bounded wait timed out.
  Timeout,
  /// The target node does not support this deployment operation.
  Unsupported,
  /// The request was structurally invalid.
  InvalidRequest,
  /// The target remote address terminated before create completed.
  AddressTerminated,
}

impl RemoteDeploymentFailureCode {
  pub(crate) const fn to_wire(self) -> u8 {
    match self {
      | Self::UnknownFactoryId => 0x01,
      | Self::DuplicateChildName => 0x02,
      | Self::DeserializationFailed => 0x03,
      | Self::SpawnFailed => 0x04,
      | Self::Timeout => 0x05,
      | Self::Unsupported => 0x06,
      | Self::InvalidRequest => 0x07,
      | Self::AddressTerminated => 0x08,
    }
  }

  pub(crate) const fn from_wire(value: u8) -> Option<Self> {
    match value {
      | 0x01 => Some(Self::UnknownFactoryId),
      | 0x02 => Some(Self::DuplicateChildName),
      | 0x03 => Some(Self::DeserializationFailed),
      | 0x04 => Some(Self::SpawnFailed),
      | 0x05 => Some(Self::Timeout),
      | 0x06 => Some(Self::Unsupported),
      | 0x07 => Some(Self::InvalidRequest),
      | 0x08 => Some(Self::AddressTerminated),
      | _ => None,
    }
  }
}
