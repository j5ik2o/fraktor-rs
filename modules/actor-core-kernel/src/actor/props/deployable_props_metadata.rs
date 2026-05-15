use alloc::string::String;

use crate::actor::messaging::AnyMessage;

/// Wire-safe deployment metadata carried by props for remote creation.
///
/// The metadata contains only a stable target-node factory id and a payload that can be handed to
/// actor-core serialization. It intentionally does not expose the local actor factory closure.
#[derive(Clone, Debug)]
pub struct DeployablePropsMetadata {
  factory_id: String,
  payload:    AnyMessage,
}

impl DeployablePropsMetadata {
  /// Creates deployable props metadata.
  #[must_use]
  pub fn new(factory_id: impl Into<String>, payload: AnyMessage) -> Self {
    Self { factory_id: factory_id.into(), payload }
  }

  /// Returns the target-node deployable factory id.
  #[must_use]
  pub fn factory_id(&self) -> &str {
    &self.factory_id
  }

  /// Returns the actor-core-serializable factory payload.
  #[must_use]
  pub const fn payload(&self) -> &AnyMessage {
    &self.payload
  }
}
