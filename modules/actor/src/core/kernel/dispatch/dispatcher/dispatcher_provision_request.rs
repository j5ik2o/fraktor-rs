use alloc::string::String;

/// Actor bootstrap request passed to dispatcher providers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatcherProvisionRequest {
  dispatcher_id: String,
  actor_name:    Option<String>,
}

impl DispatcherProvisionRequest {
  /// Creates a new dispatcher provision request.
  #[must_use]
  pub fn new(dispatcher_id: impl Into<String>) -> Self {
    Self { dispatcher_id: dispatcher_id.into(), actor_name: None }
  }

  /// Adds the logical actor name for the bootstrap request.
  #[must_use]
  pub fn with_actor_name(mut self, actor_name: impl Into<String>) -> Self {
    self.actor_name = Some(actor_name.into());
    self
  }

  /// Returns the resolved dispatcher registry identifier.
  #[must_use]
  pub fn dispatcher_id(&self) -> &str {
    &self.dispatcher_id
  }

  /// Returns the logical actor name when available.
  #[must_use]
  pub fn actor_name(&self) -> Option<&str> {
    self.actor_name.as_deref()
  }
}
