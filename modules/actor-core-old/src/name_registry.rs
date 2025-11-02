//! Name to PID registry used within actor scopes.

mod name_registry_error;
mod name_registry_struct;

pub use name_registry_error::NameRegistryError;
pub use name_registry_struct::NameRegistry;
