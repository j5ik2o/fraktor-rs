//! Extension identifier and factory.

use core::any::TypeId;

use super::Extension;
use crate::core::system::ActorSystem;

/// Identifier + factory for an [`Extension`].
pub trait ExtensionId: Send + Sync + 'static {
  /// Extension implementation type.
  type Ext: Extension;

  /// Creates a new extension instance for the provided actor system.
  fn create_extension(&self, system: &ActorSystem) -> Self::Ext;

  /// Returns the [`TypeId`] used to store and fetch this extension.
  #[must_use]
  fn id(&self) -> TypeId {
    TypeId::of::<Self>()
  }
}
