use alloc::string::String;

use crate::actor::{Pid, actor_path::ActorPath, deploy::RemoteScope, props::DeployablePropsMetadata};

/// Actor-core request passed to the installed remote deployment hook.
#[derive(Clone, Debug)]
pub struct RemoteDeploymentRequest {
  parent:              Pid,
  child_pid:           Pid,
  child_name:          String,
  child_path:          ActorPath,
  scope:               RemoteScope,
  deployable_metadata: Option<DeployablePropsMetadata>,
}

impl RemoteDeploymentRequest {
  /// Creates a remote deployment request.
  #[must_use]
  pub const fn new(
    parent: Pid,
    child_pid: Pid,
    child_name: String,
    child_path: ActorPath,
    scope: RemoteScope,
    deployable_metadata: Option<DeployablePropsMetadata>,
  ) -> Self {
    Self { parent, child_pid, child_name, child_path, scope, deployable_metadata }
  }

  /// Returns the local parent pid that requested the child spawn.
  #[must_use]
  pub const fn parent(&self) -> Pid {
    self.parent
  }

  /// Returns the local placeholder pid for this spawn request.
  #[must_use]
  pub const fn child_pid(&self) -> Pid {
    self.child_pid
  }

  /// Returns the requested child name.
  #[must_use]
  pub fn child_name(&self) -> &str {
    &self.child_name
  }

  /// Returns the local logical child path.
  #[must_use]
  pub const fn child_path(&self) -> &ActorPath {
    &self.child_path
  }

  /// Returns the remote deployment scope selected for this child.
  #[must_use]
  pub const fn scope(&self) -> &RemoteScope {
    &self.scope
  }

  /// Returns the deployable props metadata, if the props are wire-deployable.
  #[must_use]
  pub const fn deployable_metadata(&self) -> Option<&DeployablePropsMetadata> {
    self.deployable_metadata.as_ref()
  }
}
