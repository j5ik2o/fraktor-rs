//! Results returned by placement command execution.

use crate::core::{
  activation_entry::ActivationEntry, activation_error::ActivationError,
  activation_storage_error::ActivationStorageError, placement_lease::PlacementLease,
  placement_lock_error::PlacementLockError, placement_request_id::PlacementRequestId,
};

/// Result of executing a placement command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementCommandResult {
  /// Lock acquisition completed.
  LockAcquired {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Lock acquisition result.
    result:     Result<PlacementLease, PlacementLockError>,
  },
  /// Activation entry loaded.
  ActivationLoaded {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Load result.
    result:     Result<Option<ActivationEntry>, ActivationStorageError>,
  },
  /// Activation ensured.
  ActivationEnsured {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Ensure result.
    result:     Result<crate::core::activation_record::ActivationRecord, ActivationError>,
  },
  /// Activation entry stored.
  ActivationStored {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Store result.
    result:     Result<(), ActivationStorageError>,
  },
  /// Lock released.
  LockReleased {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Release result.
    result:     Result<(), PlacementLockError>,
  },
}
