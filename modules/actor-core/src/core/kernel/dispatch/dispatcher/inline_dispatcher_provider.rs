use alloc::boxed::Box;

use super::{
  configured_dispatcher_builder::ConfiguredDispatcherBuilder, dispatcher_builder::DispatcherBuilder,
  dispatcher_provider::DispatcherProvider, dispatcher_provision_request::DispatcherProvisionRequest,
  dispatcher_settings::DispatcherSettings, inline_executor::InlineExecutor,
};
use crate::core::kernel::actor::spawn::SpawnError;

/// Inline dispatcher provider used for the kernel default registry entries.
pub(crate) struct InlineDispatcherProvider;

impl InlineDispatcherProvider {
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self
  }
}

impl DispatcherProvider for InlineDispatcherProvider {
  fn provision(
    &self,
    settings: &DispatcherSettings,
    _request: &DispatcherProvisionRequest,
  ) -> Result<Box<dyn DispatcherBuilder>, SpawnError> {
    Ok(Box::new(ConfiguredDispatcherBuilder::from_executor_with_settings(
      Box::new(InlineExecutor::new()),
      settings.clone(),
    )))
  }
}
