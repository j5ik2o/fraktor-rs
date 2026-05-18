//! Remote deployment request/response PDU.

use crate::wire::{RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest, RemoteDeploymentCreateSuccess};

/// Wire PDU for remote deployment create request and response traffic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteDeploymentPdu {
  /// Request to create a deployable actor on the target node.
  CreateRequest(RemoteDeploymentCreateRequest),
  /// Successful create response with the canonical created actor path.
  CreateSuccess(RemoteDeploymentCreateSuccess),
  /// Failed create response with a structured code and reason.
  CreateFailure(RemoteDeploymentCreateFailure),
}
