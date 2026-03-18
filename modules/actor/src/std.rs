/// Actor primitives specialised for the standard toolbox.
pub mod actor {
  mod actor_adapter;
  mod actor_context;
  mod actor_lifecycle;

  pub use actor_adapter::ActorAdapter;
  pub use actor_context::ActorContext;
  pub use actor_lifecycle::Actor;
}
/// Dispatch bindings for the standard toolbox.
pub mod dispatch {
  //! Dispatch bindings specialised for the standard toolbox.

  pub mod dispatcher {
    //! Dispatcher bindings tailored for the standard runtime facade.

    mod base;
    /// Dispatch executor implementations for the standard runtime.
    pub mod dispatch_executor;
    mod dispatch_executor_adapter;
    /// Dispatcher configuration bindings tailored for the standard runtime.
    mod dispatcher_config;
    /// Pinned dispatcher that dedicates a single execution lane per actor.
    mod pinned_dispatcher;
    mod schedule_adapter;

    pub use base::*;
    pub use dispatch_executor_adapter::DispatchExecutorAdapter;
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
/// Props and dispatcher configuration bindings for the standard toolbox.
pub mod props {
  mod base;

  pub use base::*;
}
/// Scheduler bindings for the standard toolbox.
pub mod scheduler {
  //! Scheduler utilities specialised for the standard toolbox runtime.

  /// Tick driver integrations for standard runtimes.
  #[cfg(feature = "tokio-executor")]
  pub mod tick;
}
/// Actor system bindings for the standard toolbox.
pub mod system {
  mod actor_system_config;
  mod base;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_error;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_id;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_installer;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_phase;
  #[cfg(feature = "tokio-executor")]
  mod coordinated_shutdown_reason;

  pub use actor_system_config::*;
  pub use base::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_error::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_id::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_installer::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_phase::*;
  #[cfg(feature = "tokio-executor")]
  pub use coordinated_shutdown_reason::*;
}
/// Typed actor utilities specialised for the standard toolbox runtime.
pub mod typed {
  //! High-level typed actor bindings for the standard fraktor runtime.

  /// Core typed actor primitives including actors, contexts, and references.
  pub mod actor {
    mod actor_adaptor;
    mod actor_context;
    mod actor_context_ref;
    mod actor_ref;
    mod child_ref;
    mod typed_actor;

    pub use actor_adaptor::*;
    pub use actor_context::*;
    pub use actor_context_ref::*;
    pub use actor_ref::*;
    pub use child_ref::*;
    pub use typed_actor::*;
  }
  mod behaviors;
  mod log_options;
  mod props;
  mod system;

  pub use behaviors::Behaviors;
  pub use log_options::LogOptions;
  pub use props::TypedProps;
  pub use system::TypedActorSystem;
}

#[cfg(test)]
mod tests;
