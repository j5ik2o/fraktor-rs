#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, clippy::dbg_macro)]
#![deny(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Standard library helpers for Cellactor runtime integrations.

pub use cellactor_actor_core_rs::{
  actor_prim::ChildRef as GenericChildRef,
  deadletter::DeadletterGeneric as GenericDeadletter,
  error::SendError as GenericSendError,
  mailbox::Mailbox as GenericMailbox,
  messaging::AskResponse as GenericAskResponse,
  props::{
    DispatcherConfig as GenericDispatcherConfig, MailboxConfig as GenericMailboxConfig, Props as GenericProps,
    SupervisorOptions as GenericSupervisorOptions,
  },
  system::ActorSystemGeneric,
};
pub use cellactor_utils_std_rs::{StdMutex, StdMutexFamily, StdToolbox};

/// ActorSystem specialized for std environment.
pub type ActorSystem = ActorSystemGeneric<StdToolbox>;

/// Props specialized for std environment.
pub type Props = cellactor_actor_core_rs::props::Props<StdToolbox>;

/// ActorContext specialized for std environment.
pub type ActorContext<'a> = cellactor_actor_core_rs::actor_prim::ActorContext<'a, StdToolbox>;

/// ActorRef specialized for std environment.
pub type ActorRef = cellactor_actor_core_rs::actor_prim::actor_ref::ActorRef<StdToolbox>;

/// ChildRef specialized for std environment.
pub type ChildRef = cellactor_actor_core_rs::actor_prim::ChildRef<StdToolbox>;

/// AnyMessage specialized for std environment.
pub type AnyMessage = cellactor_actor_core_rs::messaging::AnyMessage<StdToolbox>;

/// AnyMessageView specialized for std environment.
pub type AnyMessageView<'a> = cellactor_actor_core_rs::messaging::AnyMessageView<'a, StdToolbox>;

/// ActorFuture specialized for std environment.
pub type ActorFuture<T> = cellactor_actor_core_rs::futures::ActorFuture<T, StdToolbox>;

/// ActorFutureListener specialized for std environment.
pub type ActorFutureListener<'a, T> = cellactor_actor_core_rs::futures::ActorFutureListener<'a, T, StdToolbox>;

/// AskResponse specialized for std environment.
pub type AskResponse = cellactor_actor_core_rs::messaging::AskResponse<StdToolbox>;

/// SendError specialized for std environment.
pub type SendError = cellactor_actor_core_rs::error::SendError<StdToolbox>;

/// Mailbox specialized for std environment.
pub type Mailbox = cellactor_actor_core_rs::mailbox::Mailbox<StdToolbox>;

/// Dispatcher specialized for std environment.
pub type Dispatcher = cellactor_actor_core_rs::system::dispatcher::Dispatcher<StdToolbox>;

/// DispatcherConfig specialized for std environment.
pub type DispatcherConfig = cellactor_actor_core_rs::props::DispatcherConfig<StdToolbox>;

/// EventStream specialized for std environment.
pub type EventStream = cellactor_actor_core_rs::eventstream::EventStreamGeneric<StdToolbox>;

/// EventStreamEvent specialized for std environment.
pub type EventStreamEvent = cellactor_actor_core_rs::eventstream::EventStreamEvent<StdToolbox>;

/// EventStreamSubscription specialized for std environment.
pub type EventStreamSubscription = cellactor_actor_core_rs::eventstream::EventStreamSubscriptionGeneric<StdToolbox>;

/// Deadletter specialized for std environment.
pub type Deadletter = cellactor_actor_core_rs::deadletter::DeadletterGeneric<StdToolbox>;

/// SystemState specialized for std environment.
pub type SystemState = cellactor_actor_core_rs::system::SystemState<StdToolbox>;
