//! Remote deployment outcomes emitted by core protocol handling.

use alloc::boxed::Box;

use crate::{
  address::Address, extension::RemoteDeploymentResponse, transport::TransportEndpoint,
  wire::RemoteDeploymentCreateRequest,
};

/// Side-effect instruction emitted by remote deployment protocol handling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteDeploymentOutcome {
  /// Ask the adapter to run the target-side deployment daemon for a create request.
  CreateRequested {
    /// Verified remote actor system that should receive the create response.
    response_remote: Address,
    /// Remote authority that submitted the request.
    authority:       TransportEndpoint,
    /// Create request to apply against the local actor system.
    request:         Box<RemoteDeploymentCreateRequest>,
    /// Monotonic millis at which the request frame was observed.
    now_ms:          u64,
  },
  /// A pending origin-side deployment request was completed.
  ResponseCompleted {
    /// Matched response.
    response: RemoteDeploymentResponse,
  },
}
