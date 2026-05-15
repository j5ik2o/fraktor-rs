use super::{RemoteDeploymentOutcome, RemoteDeploymentRequest};

/// Hook used by actor-ref providers to handle remote-scoped child deployment.
pub trait RemoteDeploymentHook: Send + 'static {
  /// Attempts to deploy the child described by `request`.
  fn deploy_child(&mut self, request: RemoteDeploymentRequest) -> RemoteDeploymentOutcome;
}
