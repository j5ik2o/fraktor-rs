//! Actor system extension traits.

#[cfg(test)]
mod tests;

mod ext;
mod extension_id;
mod extension_installer;
mod extensions_config;

pub use ext::Extension;
pub use extension_id::ExtensionId;
pub use extension_installer::ExtensionInstaller;
pub use extensions_config::ExtensionsConfig;
