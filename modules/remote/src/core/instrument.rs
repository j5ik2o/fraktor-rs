//! Instrumentation and observability for remoting: metrics, hooks, and flight recorder.

pub mod flight_recorder;
mod remote_instrument;
#[cfg(feature = "tokio-transport")]
mod remote_instruments;

pub use remote_instrument::RemoteInstrument;
#[cfg(feature = "tokio-transport")]
pub(crate) use remote_instruments::RemoteInstruments;
