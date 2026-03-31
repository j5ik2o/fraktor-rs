//! Messages and signals delivered to typed behaviors.
//!
//! Corresponds to `org.apache.pekko.actor.typed.MessageAndSignals` in the
//! Pekko reference implementation.

mod death_pact_error;
mod signal;

pub use death_pact_error::DeathPactError;
pub use signal::BehaviorSignal;
