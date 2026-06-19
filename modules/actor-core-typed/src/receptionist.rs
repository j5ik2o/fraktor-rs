//! Receptionist actor providing service discovery within an actor system.

mod deregistered;
mod extension;
mod listing;
mod receptionist_command;
mod receptionist_setup;
mod registered;
mod service_key;
#[cfg(test)]
#[path = "receptionist_test.rs"]
mod tests;

pub use deregistered::Deregistered;
pub(crate) use extension::ReceptionistExtensionId;
pub use extension::{Receptionist, SYSTEM_RECEPTIONIST_TOP_LEVEL};
pub use listing::Listing;
pub use receptionist_command::ReceptionistCommand;
pub use receptionist_setup::ReceptionistSetup;
pub use registered::Registered;
pub use service_key::ServiceKey;
