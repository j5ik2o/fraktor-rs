//! No-op implementation of [`RemoteDeploymentHook`].

use alloc::string::ToString;

use super::{RemoteDeploymentHook, RemoteDeploymentOutcome, RemoteDeploymentRequest};

/// Default hook used when no remote actor-ref provider has registered deployment support.
pub(crate) struct NoopRemoteDeploymentHook;

impl RemoteDeploymentHook for NoopRemoteDeploymentHook {
  fn deploy_child(&mut self, _request: RemoteDeploymentRequest) -> RemoteDeploymentOutcome {
    RemoteDeploymentOutcome::Failed("remote deployment hook is not installed".to_string())
  }
}
