//! Serialization extension module.

#[cfg(test)]
mod tests;

mod r#impl;
mod serialization_extension_id;

pub use r#impl::Serialization;
pub use serialization_extension_id::{SERIALIZATION_EXTENSION, SerializationExtensionId};
