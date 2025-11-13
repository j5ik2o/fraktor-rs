/// Priority assigned to shutdown tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRunPriority {
  /// Executed before all other tasks.
  SystemCritical,
  /// Executed after system-critical tasks.
  Runtime,
  /// Executed last.
  User,
}

impl TaskRunPriority {
  pub(crate) const fn rank(self) -> u8 {
    match self {
      | Self::SystemCritical => 2,
      | Self::Runtime => 1,
      | Self::User => 0,
    }
  }
}
