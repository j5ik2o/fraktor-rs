use crate::actor::{
  messaging::AnyMessage,
  props::{DeployableFactoryError, Props},
};

/// Target-node contract that converts deployable payloads into local actor props.
///
/// Implementations live on the target node and are looked up by stable factory id.
/// The factory itself is never serialized across the wire.
pub trait DeployableActorFactory: Send + Sync + 'static {
  /// Builds local actor props from the deserialized deployment payload.
  ///
  /// # Errors
  ///
  /// Returns [`DeployableFactoryError`] when the payload is not accepted by this factory.
  fn props_for_payload(&self, payload: AnyMessage) -> Result<Props, DeployableFactoryError>;
}

impl<F> DeployableActorFactory for F
where
  F: Fn(AnyMessage) -> Result<Props, DeployableFactoryError> + Send + Sync + 'static,
{
  fn props_for_payload(&self, payload: AnyMessage) -> Result<Props, DeployableFactoryError> {
    self(payload)
  }
}
