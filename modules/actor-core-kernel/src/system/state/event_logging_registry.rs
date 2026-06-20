//! Event stream and logging state owned by SystemState.

#[cfg(test)]
#[path = "event_logging_registry_test.rs"]
mod tests;

use alloc::boxed::Box;

use portable_atomic::AtomicU64;

use crate::{
  actor::actor_ref::dead_letter::DeadLetterShared,
  event::{
    logging::{DefaultLoggingFilter, LoggingFilter},
    stream::{EventStream, EventStreamShared},
  },
};

/// Owns event stream, dead letters, logging, and failure telemetry.
pub(crate) struct EventLoggingRegistry {
  pub(crate) event_stream:           EventStreamShared,
  pub(crate) dead_letter:            DeadLetterShared,
  pub(crate) logging_filter:         Box<dyn LoggingFilter>,
  pub(crate) failure_total:          AtomicU64,
  pub(crate) failure_restart_total:  AtomicU64,
  pub(crate) failure_stop_total:     AtomicU64,
  pub(crate) failure_escalate_total: AtomicU64,
  pub(crate) failure_resume_total:   AtomicU64,
  pub(crate) failure_inflight:       AtomicU64,
}

impl EventLoggingRegistry {
  pub(crate) fn with_capacities(dead_letter_capacity: usize, event_stream_capacity: usize) -> Self {
    let event_stream = EventStreamShared::new(EventStream::with_capacity(event_stream_capacity));
    let dead_letter = DeadLetterShared::with_capacity(event_stream.clone(), dead_letter_capacity);
    Self::new(event_stream, dead_letter)
  }

  pub(crate) fn new(event_stream: EventStreamShared, dead_letter: DeadLetterShared) -> Self {
    Self {
      event_stream,
      dead_letter,
      logging_filter: Box::new(DefaultLoggingFilter::default()),
      failure_total: AtomicU64::new(0),
      failure_restart_total: AtomicU64::new(0),
      failure_stop_total: AtomicU64::new(0),
      failure_escalate_total: AtomicU64::new(0),
      failure_resume_total: AtomicU64::new(0),
      failure_inflight: AtomicU64::new(0),
    }
  }
}
