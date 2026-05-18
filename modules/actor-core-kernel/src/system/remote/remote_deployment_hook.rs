use super::{RemoteDeploymentOutcome, RemoteDeploymentRequest};

/// Hook used by actor-ref providers to handle remote-scoped child deployment.
pub trait RemoteDeploymentHook: Send + Sync + 'static {
  /// Attempts to deploy the child described by `request`.
  fn deploy_child(&self, request: RemoteDeploymentRequest) -> RemoteDeploymentOutcome;
}
