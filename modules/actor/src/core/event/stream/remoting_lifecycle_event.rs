//! Remoting lifecycle notifications pushed through the event stream.

#[cfg(test)]
mod tests;

use alloc::string::String;

use super::{
  correlation_id::CorrelationId, graceful_shutdown_quarantined_event::GracefulShutdownQuarantinedEvent,
  this_actor_system_quarantined_event::ThisActorSystemQuarantinedEvent,
};

/// Lifecycle event emitted by the remoting subsystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemotingLifecycleEvent {
  /// Remoting is preparing to start.
  Starting,
  /// Remoting finished startup procedures.
  Started,
  /// Transport listener is now bound to the canonical authority.
  ListenStarted {
    /// Authority (usually `host:port`) associated with the listener.
    authority:      String,
    /// Correlation identifier shared with transports/flight recorder.
    correlation_id: CorrelationId,
  },
  /// Association transitioned into the connected state.
  Connected {
    /// Authority (usually `host:port`) participating in the association.
    authority:      String,
    /// Remote system identifier.
    remote_system:  String,
    /// Remote actor system UID.
    remote_uid:     u64,
    /// Correlation identifier linking transport level diagnostics.
    correlation_id: CorrelationId,
  },
  /// Authority moved into quarantine.
  Quarantined {
    /// Authority currently quarantined.
    authority:      String,
    /// Describes why the quarantine was triggered.
    reason:         String,
    /// Correlation identifier linking to deferred queue drains and metrics.
    correlation_id: CorrelationId,
  },
  /// Authority was quarantined due to graceful shutdown.
  GracefulShutdownQuarantined(GracefulShutdownQuarantinedEvent),
  /// This actor system was quarantined by the specified remote authority.
  ThisActorSystemQuarantined(ThisActorSystemQuarantinedEvent),
  /// Authority temporarily gated following a transient transport failure.
  Gated {
    /// Authority for which gating was applied.
    authority:      String,
    /// Correlation identifier assigned to the gating event.
    correlation_id: CorrelationId,
  },
  /// Remoting is shutting down or already stopped.
  Shutdown,
  /// Remoting encountered a fatal error.
  Error {
    /// Describes the error that forced remoting to stop.
    message: String,
  },
}
