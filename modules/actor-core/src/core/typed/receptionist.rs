//! Receptionist actor providing service discovery within an actor system.

mod deregistered;
mod listing;
mod receptionist_command;
mod registered;
mod runtime;
mod service_key;
#[cfg(test)]
mod tests;

pub use deregistered::Deregistered;
pub use listing::Listing;
pub use receptionist_command::ReceptionistCommand;
pub use registered::Registered;
pub use runtime::{Receptionist, SYSTEM_RECEPTIONIST_TOP_LEVEL};
pub use service_key::ServiceKey;
