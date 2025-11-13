/// Handle returned when registering shutdown tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskRunHandle {
  id: u64,
}

impl TaskRunHandle {
  /// Creates a new handle from the provided identifier.
  #[must_use]
  pub const fn new(id: u64) -> Self {
    Self { id }
  }

  /// Returns the numeric identifier for the handle.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }
}
