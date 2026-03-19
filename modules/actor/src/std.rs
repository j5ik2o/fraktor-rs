/// Dispatch bindings for the standard toolbox.
pub mod dispatch {
  //! Dispatch bindings specialised for the standard toolbox.

  pub mod dispatcher {
    //! Dispatcher bindings tailored for the standard runtime facade.

    /// Dispatch executor implementations for the standard runtime.
    pub mod dispatch_executor;
    /// Dispatcher configuration bindings tailored for the standard runtime.
    mod dispatcher_config;
    /// Pinned dispatcher that dedicates a single execution lane per actor.
    mod pinned_dispatcher;
    mod schedule_adapter;

    pub use dispatcher_config::DispatcherConfig;
    pub use pinned_dispatcher::PinnedDispatcher;
    pub use schedule_adapter::StdScheduleAdapter;
  }
}
/// Event bindings for the standard toolbox.
pub mod event {
  //! Event-related bindings for standard runtimes.

  pub mod logging {
    //! Logging bindings for standard runtimes.

    mod tracing_logger_subscriber;

    pub use tracing_logger_subscriber::TracingLoggerSubscriber;
  }

  pub mod stream {
    //! Event stream bindings for standard runtimes.

    mod dead_letter_log_subscriber;
    mod subscriber;
    mod subscriber_adapter;

    pub use dead_letter_log_subscriber::DeadLetterLogSubscriber;
    pub use subscriber::{EventStreamSubscriber, EventStreamSubscriberShared, subscriber_handle};
    pub use subscriber_adapter::*;
  }
}
/// Pekko-inspired helper patterns for the standard toolbox.
pub mod pattern;
/// Scheduler bindings for the standard toolbox.
pub mod scheduler {
  //! Scheduler utilities specialised for the standard toolbox runtime.

  /// Tick driver integrations for standard runtimes.
  #[cfg(feature = "tokio-executor")]
  pub mod tick;
}
/// Actor system bindings for the standard toolbox.
// NOTE: CoordinatedShutdownPhase と CoordinatedShutdownReason は core::system に移設済み。
// no-parent-reexport lint により std からの re-export は禁止されているため、
// 利用者は crate::core::system::{CoordinatedShutdownPhase, CoordinatedShutdownReason} を直接参照すること。
pub mod system {
  mod base;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_error;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_id;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_installer;

  pub use base::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_error::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_id::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_installer::*;
}
/// Typed actor utilities specialised for the standard toolbox runtime.
pub mod typed {
  //! High-level typed actor bindings for the standard fraktor runtime.

  mod behaviors;
  mod log_options;

  pub use behaviors::Behaviors;
  pub use log_options::LogOptions;
}

#[cfg(test)]
mod tests;
