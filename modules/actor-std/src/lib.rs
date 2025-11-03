#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, clippy::dbg_macro)]
#![deny(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Standard library helpers for Cellactor runtime integrations.

pub use cellactor_actor_core_rs::{
  ActorSystemGeneric, AskResponse as GenericAskResponse, ChildRef as GenericChildRef,
  DeadletterGeneric as GenericDeadletter, DispatcherConfig as GenericDispatcherConfig, Mailbox as GenericMailbox,
  MailboxConfig as GenericMailboxConfig, Props as GenericProps, SendError as GenericSendError,
  SupervisorOptions as GenericSupervisorOptions,
};
pub use cellactor_utils_std_rs::{StdMutex, StdMutexFamily, StdToolbox};

/// ActorSystem specialized for std environment.
pub type ActorSystem = ActorSystemGeneric<StdToolbox>;

/// Props specialized for std environment.
pub type Props = cellactor_actor_core_rs::Props<StdToolbox>;

/// ActorContext specialized for std environment.
pub type ActorContext<'a> = cellactor_actor_core_rs::ActorContext<'a, StdToolbox>;

/// ActorRef specialized for std environment.
pub type ActorRef = cellactor_actor_core_rs::ActorRef<StdToolbox>;

/// ChildRef specialized for std environment.
pub type ChildRef = cellactor_actor_core_rs::ChildRef<StdToolbox>;

/// AnyMessage specialized for std environment.
pub type AnyMessage = cellactor_actor_core_rs::AnyMessage<StdToolbox>;

/// AnyMessageView specialized for std environment.
pub type AnyMessageView<'a> = cellactor_actor_core_rs::AnyMessageView<'a, StdToolbox>;

/// ActorFuture specialized for std environment.
pub type ActorFuture<T> = cellactor_actor_core_rs::ActorFuture<T, StdToolbox>;

/// ActorFutureListener specialized for std environment.
pub type ActorFutureListener<'a, T> = cellactor_actor_core_rs::ActorFutureListener<'a, T, StdToolbox>;

/// AskResponse specialized for std environment.
pub type AskResponse = cellactor_actor_core_rs::AskResponse<StdToolbox>;

/// SendError specialized for std environment.
pub type SendError = cellactor_actor_core_rs::SendError<StdToolbox>;

/// Mailbox specialized for std environment.
pub type Mailbox = cellactor_actor_core_rs::Mailbox<StdToolbox>;

/// Dispatcher specialized for std environment.
pub type Dispatcher = cellactor_actor_core_rs::Dispatcher<StdToolbox>;

/// DispatcherConfig specialized for std environment.
pub type DispatcherConfig = cellactor_actor_core_rs::DispatcherConfig<StdToolbox>;

/// EventStream specialized for std environment.
pub type EventStream = cellactor_actor_core_rs::EventStreamGeneric<StdToolbox>;

/// EventStreamEvent specialized for std environment.
pub type EventStreamEvent = cellactor_actor_core_rs::EventStreamEvent<StdToolbox>;

/// EventStreamSubscription specialized for std environment.
pub type EventStreamSubscription = cellactor_actor_core_rs::EventStreamSubscriptionGeneric<StdToolbox>;

/// Deadletter specialized for std environment.
pub type Deadletter = cellactor_actor_core_rs::DeadletterGeneric<StdToolbox>;

/// SystemState specialized for std environment.
pub type SystemState = cellactor_actor_core_rs::SystemState<StdToolbox>;
