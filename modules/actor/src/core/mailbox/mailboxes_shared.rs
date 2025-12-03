//! Shared wrapper for mailbox registry.

use alloc::string::String;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::MailboxesGeneric;
use crate::core::{mailbox::MailboxRegistryError, props::MailboxConfig};

/// Shared wrapper for [`MailboxesGeneric`] enabling interior mutability.
///
/// This wrapper provides `&self` methods that internally lock the underlying
/// [`MailboxesGeneric`], allowing safe concurrent access from multiple owners.
pub struct MailboxesSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<MailboxesGeneric<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for MailboxesSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for MailboxesSharedGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> MailboxesSharedGeneric<TB> {
  /// Creates a new shared mailbox registry.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(MailboxesGeneric::new())) }
  }

  /// Registers a mailbox configuration.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Duplicate`] when the identifier already exists.
  pub fn register(&self, id: impl Into<String>, config: MailboxConfig) -> Result<(), MailboxRegistryError> {
    self.inner.lock().register(id, config)
  }

  /// Resolves the mailbox configuration for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.inner.lock().resolve(id)
  }

  /// Ensures the default mailbox configuration is registered.
  pub fn ensure_default(&self) {
    self.inner.lock().ensure_default();
  }
}

/// Type alias for [`MailboxesSharedGeneric`] using the default [`NoStdToolbox`].
pub type MailboxesShared<TB> = MailboxesSharedGeneric<TB>;
