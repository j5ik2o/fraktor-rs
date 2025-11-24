//! Helper provider used in tests to inject failures.

use super::{ClusterProvider, ClusterProviderError};

#[derive(Clone, Debug)]
pub(super) struct FailingProvider {
  start_member_error: Option<ClusterProviderError>,
  start_client_error: Option<ClusterProviderError>,
  shutdown_error:     Option<ClusterProviderError>,
}

impl FailingProvider {
  pub(super) fn member_fail(reason: impl Into<String>) -> Self {
    Self {
      start_member_error: Some(ClusterProviderError::start_member(reason)),
      start_client_error: None,
      shutdown_error:     None,
    }
  }

  pub(super) fn client_fail(reason: impl Into<String>) -> Self {
    Self {
      start_member_error: None,
      start_client_error: Some(ClusterProviderError::start_client(reason)),
      shutdown_error:     None,
    }
  }

  pub(super) fn shutdown_fail(reason: impl Into<String>) -> Self {
    Self {
      start_member_error: None,
      start_client_error: None,
      shutdown_error:     Some(ClusterProviderError::shutdown(reason)),
    }
  }
}

impl ClusterProvider for FailingProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.start_member_error {
      return Err(err.clone());
    }
    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.start_client_error {
      return Err(err.clone());
    }
    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.shutdown_error {
      return Err(err.clone());
    }
    Ok(())
  }
}
