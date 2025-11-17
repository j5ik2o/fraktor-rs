//! Extension identifier and factory.

use core::any::TypeId;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::Extension;
use crate::core::system::ActorSystemGeneric;

/// Identifier + factory for an [`Extension`].
pub trait ExtensionId<TB: RuntimeToolbox>: Send + Sync + 'static {
  /// Extension implementation type.
  type Ext: Extension<TB>;

  /// Creates a new extension instance for the provided actor system.
  fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext;

  /// Returns the [`TypeId`] used to store and fetch this extension.
  #[must_use]
  fn id(&self) -> TypeId {
    TypeId::of::<Self>()
  }
}
