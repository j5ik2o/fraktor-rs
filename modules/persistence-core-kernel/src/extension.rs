//! Persistence extension package.

mod persistence_extension;
mod persistence_extension_id;
mod persistence_extension_installer;
mod persistence_extension_shared;

pub use persistence_extension::PersistenceExtension;
pub use persistence_extension_id::PersistenceExtensionId;
pub use persistence_extension_installer::PersistenceExtensionInstaller;
pub use persistence_extension_shared::PersistenceExtensionShared;
