//! std Split Brain Resolver provider binding.

#[cfg(test)]
#[path = "split_brain_resolver_provider_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::time::Duration;

use fraktor_cluster_core_kernel_rs::{
  downing_provider::{
    DowningDecision, DowningDecisionContext, DowningInput, DowningProvider, DowningProviderCompatibility,
    LeaseAcquisitionOutcome, LeaseMajorityPort, SplitBrainResolverProviderHook, SplitBrainResolverSettings,
  },
  extension::ClusterProviderError,
};
use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

use super::StdLeaseMajorityBackend;

const NOT_STARTED: &str = "split-brain-resolver provider is not started";
const ALREADY_STARTED: &str = "split-brain-resolver provider is already started";

type LeaseBackendFactory = ArcShared<dyn Fn() -> Box<dyn StdLeaseMajorityBackend> + Send + Sync>;

/// std lifecycle wrapper for the core Split Brain Resolver provider hook.
pub struct StdSplitBrainResolverProvider {
  settings:        SplitBrainResolverSettings,
  hook:            Option<SplitBrainResolverProviderHook>,
  lease_backend_f: Option<LeaseBackendFactory>,
  lease_backend:   Option<Box<dyn StdLeaseMajorityBackend>>,
  stopped:         bool,
}

impl StdSplitBrainResolverProvider {
  /// Creates a stopped provider with SBR settings.
  #[must_use]
  pub const fn new(settings: SplitBrainResolverSettings) -> Self {
    Self { settings, hook: None, lease_backend_f: None, lease_backend: None, stopped: false }
  }

  /// Configures a lease backend factory used when the provider starts.
  #[must_use]
  pub fn with_lease_backend_factory<F>(mut self, factory: F) -> Self
  where
    F: Fn() -> Box<dyn StdLeaseMajorityBackend> + Send + Sync + 'static, {
    self.lease_backend_f = Some(ArcShared::new(factory));
    self
  }

  /// Returns compatibility metadata for this provider binding.
  #[must_use]
  pub fn compatibility(&self) -> DowningProviderCompatibility {
    SplitBrainResolverProviderHook::new(self.settings).compatibility()
  }

  /// Returns true when the provider has an active core hook.
  #[must_use]
  pub const fn is_started(&self) -> bool {
    self.hook.is_some()
  }

  /// Starts the provider lifecycle.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError::DownFailed`] when the provider is already started.
  pub fn start(&mut self) -> Result<(), ClusterProviderError> {
    if self.is_started() {
      return Err(ClusterProviderError::down(ALREADY_STARTED));
    }
    self.activate();
    self.stopped = false;
    Ok(())
  }

  /// Stops the provider lifecycle and closes active backend state.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError::DownFailed`] when the provider is not started.
  pub fn stop(&mut self) -> Result<(), ClusterProviderError> {
    if !self.is_started() {
      return Err(ClusterProviderError::down(NOT_STARTED));
    }
    self.close_active();
    self.stopped = true;
    Ok(())
  }

  /// Decides from a prebuilt context while preserving std lifecycle ownership.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError::DownFailed`] when the provider is stopped or the core hook
  /// reports a decision failure.
  pub fn decide_context(&mut self, context: &DowningDecisionContext) -> Result<DowningDecision, ClusterProviderError> {
    let hook = self.hook.as_mut().ok_or_else(|| ClusterProviderError::down(NOT_STARTED))?;
    if let Some(lease_backend) = self.lease_backend.as_mut() {
      let mut lease_port = StdLeaseMajorityPort { backend: lease_backend.as_mut() };
      return hook.decide_context_with_lease(context, &mut lease_port);
    }
    hook.decide_context(context)
  }

  fn close_active(&mut self) {
    self.hook = None;
    if let Some(mut lease_backend) = self.lease_backend.take() {
      lease_backend.close();
    }
  }

  fn activate(&mut self) {
    self.hook = Some(SplitBrainResolverProviderHook::new(self.settings));
    self.lease_backend = self.lease_backend_f.as_ref().map(|factory| factory());
  }

  fn ensure_started_for_downing_provider(&mut self) -> Result<(), ClusterProviderError> {
    if self.is_started() {
      return Ok(());
    }
    if self.stopped {
      return Err(ClusterProviderError::down(NOT_STARTED));
    }
    self.activate();
    Ok(())
  }
}

impl DowningProvider for StdSplitBrainResolverProvider {
  fn decide(&mut self, input: &DowningInput) -> Result<DowningDecision, ClusterProviderError> {
    self.ensure_started_for_downing_provider()?;
    let context = DowningDecisionContext::from_downing_input(input, Self::evaluation_time());
    StdSplitBrainResolverProvider::decide_context(self, &context)
  }

  fn decide_context(&mut self, context: &DowningDecisionContext) -> Result<DowningDecision, ClusterProviderError> {
    self.ensure_started_for_downing_provider()?;
    StdSplitBrainResolverProvider::decide_context(self, context)
  }
}

impl Drop for StdSplitBrainResolverProvider {
  fn drop(&mut self) {
    self.close_active();
  }
}

struct StdLeaseMajorityPort<'a> {
  backend: &'a mut dyn StdLeaseMajorityBackend,
}

impl LeaseMajorityPort for StdLeaseMajorityPort<'_> {
  fn acquire_majority(&mut self, context: &DowningDecisionContext) -> LeaseAcquisitionOutcome {
    self.backend.acquire(context)
  }
}

impl StdSplitBrainResolverProvider {
  const fn evaluation_time() -> TimerInstant {
    TimerInstant::zero(Duration::from_millis(1))
  }
}
