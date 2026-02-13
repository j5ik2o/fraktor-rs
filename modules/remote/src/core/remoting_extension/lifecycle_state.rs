use super::error::RemotingError;

pub(super) struct RemotingLifecycleState {
  phase: LifecyclePhase,
}

impl RemotingLifecycleState {
  #[allow(dead_code)]
  pub(super) const fn new() -> Self {
    Self { phase: LifecyclePhase::Idle }
  }

  pub(super) fn is_running(&self) -> bool {
    matches!(self.phase, LifecyclePhase::Running)
  }

  pub(super) fn ensure_running(&self) -> Result<(), RemotingError> {
    match self.phase {
      | LifecyclePhase::Running => Ok(()),
      | LifecyclePhase::Idle => Err(RemotingError::NotStarted),
      | LifecyclePhase::Stopped => Err(RemotingError::AlreadyShutdown),
    }
  }

  pub(super) fn transition_to_start(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | LifecyclePhase::Idle => {
        self.phase = LifecyclePhase::Running;
        Ok(())
      },
      | LifecyclePhase::Running => Err(RemotingError::AlreadyStarted),
      | LifecyclePhase::Stopped => Err(RemotingError::AlreadyShutdown),
    }
  }

  pub(super) fn transition_to_shutdown(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | LifecyclePhase::Idle | LifecyclePhase::Running => {
        self.phase = LifecyclePhase::Stopped;
        Ok(())
      },
      | LifecyclePhase::Stopped => Err(RemotingError::AlreadyShutdown),
    }
  }

  #[allow(dead_code)]
  pub(super) fn mark_shutdown(&mut self) -> bool {
    if matches!(self.phase, LifecyclePhase::Stopped) {
      false
    } else {
      self.phase = LifecyclePhase::Stopped;
      true
    }
  }
}

#[allow(dead_code)]
enum LifecyclePhase {
  Idle,
  Running,
  Stopped,
}
