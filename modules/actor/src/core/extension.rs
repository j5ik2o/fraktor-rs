//! Actor system extension traits.

#[cfg(test)]
mod tests;

mod ext;
mod extension_id;
mod extension_installer;
mod extension_installers;

pub use ext::Extension;
pub use extension_id::ExtensionId;
pub use extension_installer::ExtensionInstaller;
pub use extension_installers::ExtensionInstallers;
