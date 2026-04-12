use alloc::string::ToString;
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SharedLock};

use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorCellState, ActorCellStateShared, ActorCellStateSharedFactory, ActorContext,
    ActorLockFactory, ActorShared, ActorSharedFactory, Pid, ReceiveTimeoutState, ReceiveTimeoutStateShared,
    ReceiveTimeoutStateSharedFactory,
    actor_path::ActorPathScheme,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory, SendOutcome},
    actor_ref_provider::{ActorRefProviderHandleShared, ActorRefProviderHandleSharedFactory, LocalActorRefProvider},
    context_pipe::{ContextPipeWakerHandle, ContextPipeWakerHandleShared, ContextPipeWakerHandleSharedFactory},
    error::{ActorError, SendError},
    messaging::{
      AnyMessage, AnyMessageView, AskResult,
      message_invoker::{MessageInvoker, MessageInvokerShared, MessageInvokerSharedFactory},
    },
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{
        ManualTestDriver, TickDriverConfig, TickDriverControl, TickDriverControlShared, TickDriverControlSharedFactory,
      },
    },
    setup::ActorSystemConfig,
  },
  dispatch::{
    dispatcher::{
      Executor, ExecutorShared, ExecutorSharedFactory, MessageDispatcher, MessageDispatcherShared,
      MessageDispatcherSharedFactory, SharedMessageQueue, SharedMessageQueueFactory, TrampolineState,
    },
    mailbox::{
      BoundedPriorityMessageQueueState, BoundedPriorityMessageQueueStateShared,
      BoundedPriorityMessageQueueStateSharedFactory, UnboundedPriorityMessageQueueState,
      UnboundedPriorityMessageQueueStateShared, UnboundedPriorityMessageQueueStateSharedFactory,
    },
  },
  event::stream::{
    EventStream, EventStreamShared, EventStreamSharedFactory, EventStreamSubscriber, EventStreamSubscriberShared,
    EventStreamSubscriberSharedFactory,
  },
  system::{
    remote::RemotingConfig,
    shared_factory::{BuiltinSpinSharedFactory, MailboxSharedSet, MailboxSharedSetFactory},
    state::{SystemStateShared, system_state::SystemState},
  },
  util::futures::{ActorFuture, ActorFutureShared, ActorFutureSharedFactory},
};

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

/// `try_tell` succeeds when the underlying sender accepts the message.
#[test]
fn try_tell_delegates_to_sender() {
  let pid = Pid::new(5, 1);
  let mut reference: ActorRef = ActorRef::new_with_builtin_lock(pid, TestSender);
  assert!(reference.try_tell(AnyMessage::new("ping")).is_ok());
}

/// `try_tell` on a null sender reports `Closed`.
#[test]
fn try_tell_on_null_sender_returns_closed() {
  let mut reference: ActorRef = ActorRef::null();
  assert!(matches!(reference.try_tell(AnyMessage::new("ping")), Err(SendError::Closed(_))));
}

/// `try_tell` on a failing sender returns the underlying send error.
#[test]
fn try_tell_on_failing_sender_returns_error() {
  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let pid = Pid::new(10, 1);
  let mut reference: ActorRef = ActorRef::new_with_builtin_lock(pid, FailingSender);
  assert!(matches!(reference.try_tell(AnyMessage::new("will-fail")), Err(SendError::Closed(_))));
}

/// `try_tell` is a hidden fallible send helper used by infrastructure code such as `ask`.
/// It returns `Result<(), SendError>` so that `ask` can propagate send failures.
#[test]
fn try_tell_returns_result_on_success() {
  let pid = Pid::new(5, 1);
  let mut reference: ActorRef = ActorRef::new_with_builtin_lock(pid, TestSender);
  assert!(reference.try_tell(AnyMessage::new("ask-payload")).is_ok());
}

/// `try_tell` propagates the error when the sender fails.
#[test]
fn try_tell_returns_error_on_failure() {
  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let pid = Pid::new(10, 1);
  let mut reference: ActorRef = ActorRef::new_with_builtin_lock(pid, FailingSender);
  assert!(matches!(reference.try_tell(AnyMessage::new("will-fail")), Err(SendError::Closed(_))));
}

/// `ask` はレスポンスハンドルを返し、結果は future 側で観測する。
#[test]
fn ask_returns_response_handle() {
  let pid = Pid::new(5, 1);
  let mut reference: ActorRef = ActorRef::new_with_builtin_lock(pid, TestSender);
  let _response = reference.ask(AnyMessage::new("ask-payload"));
}

/// `ask` on a failing sender completes the future with `SendFailed`.
#[test]
fn ask_on_failing_sender_completes_future_with_send_failed() {
  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let pid = Pid::new(10, 1);
  let mut reference: ActorRef = ActorRef::new_with_builtin_lock(pid, FailingSender);
  let response = reference.ask(AnyMessage::new("will-fail"));
  assert_ne!(response.sender().pid(), pid);
  let result = response.future().with_write(|future| future.try_take()).expect("future should be ready");
  assert!(matches!(result, Err(crate::core::kernel::actor::messaging::AskError::SendFailed(_))));
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct CountingAskSharedFactory {
  inner: BuiltinSpinSharedFactory,
  actor_ref_sender_shared_calls: ArcShared<AtomicUsize>,
  actor_future_shared_calls: ArcShared<AtomicUsize>,
}

impl CountingAskSharedFactory {
  fn new() -> (ArcShared<AtomicUsize>, ArcShared<AtomicUsize>, Self) {
    let actor_ref_sender_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let actor_future_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let provider = Self {
      inner: BuiltinSpinSharedFactory::new(),
      actor_ref_sender_shared_calls: actor_ref_sender_shared_calls.clone(),
      actor_future_shared_calls: actor_future_shared_calls.clone(),
    };
    (actor_ref_sender_shared_calls, actor_future_shared_calls, provider)
  }
}

impl ActorLockFactory for CountingAskSharedFactory {
  fn create_lock<T>(&self, value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    self.inner.create_lock(value)
  }
}

impl MessageDispatcherSharedFactory for CountingAskSharedFactory {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherSharedFactory::create_message_dispatcher_shared(&self.inner, dispatcher)
  }
}

impl ExecutorSharedFactory for CountingAskSharedFactory {
  fn create_executor_shared(&self, executor: Box<dyn Executor>, trampoline: TrampolineState) -> ExecutorShared {
    ExecutorSharedFactory::create_executor_shared(&self.inner, executor, trampoline)
  }
}

impl ActorRefSenderSharedFactory for CountingAskSharedFactory {
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    self.actor_ref_sender_shared_calls.fetch_add(1, Ordering::SeqCst);
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&self.inner, sender)
  }
}

impl ActorSharedFactory for CountingAskSharedFactory {
  fn create(&self, actor: Box<dyn Actor + Send>) -> ActorShared {
    ActorSharedFactory::create(&self.inner, actor)
  }
}

impl BoundedPriorityMessageQueueStateSharedFactory for CountingAskSharedFactory {
  fn create_bounded_priority_message_queue_state_shared(
    &self,
    state: BoundedPriorityMessageQueueState,
  ) -> BoundedPriorityMessageQueueStateShared {
    BoundedPriorityMessageQueueStateSharedFactory::create_bounded_priority_message_queue_state_shared(
      &self.inner,
      state,
    )
  }
}

impl UnboundedPriorityMessageQueueStateSharedFactory for CountingAskSharedFactory {
  fn create_unbounded_priority_message_queue_state_shared(
    &self,
    state: UnboundedPriorityMessageQueueState,
  ) -> UnboundedPriorityMessageQueueStateShared {
    UnboundedPriorityMessageQueueStateSharedFactory::create_unbounded_priority_message_queue_state_shared(
      &self.inner,
      state,
    )
  }
}

impl ActorCellStateSharedFactory for CountingAskSharedFactory {
  fn create_actor_cell_state_shared(&self, state: ActorCellState) -> ActorCellStateShared {
    ActorCellStateSharedFactory::create_actor_cell_state_shared(&self.inner, state)
  }
}

impl ReceiveTimeoutStateSharedFactory for CountingAskSharedFactory {
  fn create_receive_timeout_state_shared(&self, state: Option<ReceiveTimeoutState>) -> ReceiveTimeoutStateShared {
    ReceiveTimeoutStateSharedFactory::create_receive_timeout_state_shared(&self.inner, state)
  }
}

impl MessageInvokerSharedFactory for CountingAskSharedFactory {
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    MessageInvokerSharedFactory::create(&self.inner, invoker)
  }
}

impl SharedMessageQueueFactory for CountingAskSharedFactory {
  fn create(&self) -> SharedMessageQueue {
    SharedMessageQueueFactory::create(&self.inner)
  }
}

impl EventStreamSharedFactory for CountingAskSharedFactory {
  fn create(&self, stream: EventStream) -> EventStreamShared {
    EventStreamSharedFactory::create(&self.inner, stream)
  }
}

impl EventStreamSubscriberSharedFactory for CountingAskSharedFactory {
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared {
    EventStreamSubscriberSharedFactory::create(&self.inner, subscriber)
  }
}

impl MailboxSharedSetFactory for CountingAskSharedFactory {
  fn create(&self) -> MailboxSharedSet {
    MailboxSharedSetFactory::create(&self.inner)
  }
}

impl ActorFutureSharedFactory<AskResult> for CountingAskSharedFactory {
  fn create_actor_future_shared(&self, future: ActorFuture<AskResult>) -> ActorFutureShared<AskResult> {
    self.actor_future_shared_calls.fetch_add(1, Ordering::SeqCst);
    ActorFutureSharedFactory::create_actor_future_shared(&self.inner, future)
  }
}

impl TickDriverControlSharedFactory for CountingAskSharedFactory {
  fn create_tick_driver_control_shared(&self, control: Box<dyn TickDriverControl>) -> TickDriverControlShared {
    TickDriverControlSharedFactory::create_tick_driver_control_shared(&self.inner, control)
  }
}

impl ActorRefProviderHandleSharedFactory<LocalActorRefProvider> for CountingAskSharedFactory {
  fn create_actor_ref_provider_handle_shared(
    &self,
    provider: LocalActorRefProvider,
  ) -> ActorRefProviderHandleShared<LocalActorRefProvider> {
    ActorRefProviderHandleSharedFactory::create_actor_ref_provider_handle_shared(&self.inner, provider)
  }
}

impl ContextPipeWakerHandleSharedFactory for CountingAskSharedFactory {
  fn create_context_pipe_waker_handle_shared(&self, handle: ContextPipeWakerHandle) -> ContextPipeWakerHandleShared {
    ContextPipeWakerHandleSharedFactory::create_context_pipe_waker_handle_shared(&self.inner, handle)
  }
}

/// Builds an ActorRef with an associated SystemState.
///
/// Returns both the ActorRef and the SystemStateShared to keep the system state alive.
/// Since ActorRef now uses weak references to SystemState, the returned SystemStateShared
/// must be kept alive for the ActorRef's path methods to work.
fn build_actor_ref_with_system(remoting: Option<RemotingConfig>) -> (ActorRef, SystemStateShared) {
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let mut config = ActorSystemConfig::default()
    .with_system_name("canonical-test")
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver);
  if let Some(remoting_config) = remoting {
    config = config.with_remoting_config(remoting_config);
  }
  let state = SystemStateShared::new(SystemState::build_from_config(&config).expect("state"));

  let props = Props::from_fn(|| NoopActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root cell");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("child cell");
  state.register_cell(child.clone());

  (child.actor_ref(), state)
}

#[test]
fn canonical_path_uses_canonical_authority_when_available() {
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let (reference, _state) = build_actor_ref_with_system(Some(remoting));

  let canonical = reference.canonical_path().expect("canonical path");
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(canonical.parts().authority_endpoint(), Some("example.com:2552".to_string()));
  assert_eq!(canonical.to_relative_string(), "/user/worker");

  let local = reference.path().expect("local path");
  assert_eq!(local.parts().authority_endpoint(), None);
}

#[test]
fn canonical_path_returns_local_when_remoting_disabled() {
  let (reference, _state) = build_actor_ref_with_system(None);

  let canonical = reference.canonical_path().expect("canonical path");
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::Fraktor);
  assert_eq!(canonical.parts().authority_endpoint(), None);
  assert_eq!(canonical.to_relative_string(), "/user/worker");

  let local = reference.path().expect("local path");
  assert_eq!(local.parts().authority_endpoint(), None);
}

#[test]
fn canonical_path_is_none_without_system_state() {
  let reference: ActorRef = ActorRef::new_with_builtin_lock(Pid::new(1, 0), TestSender);
  assert!(reference.canonical_path().is_none());
}

#[test]
fn ask_without_path_aware_reply_uses_system_sender_factory_when_system_is_available() {
  let (sender_calls, future_calls, provider) = CountingAskSharedFactory::new();
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::default()
    .with_system_name("ask-shared-factory")
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_shared_factory(provider);
  let state = SystemStateShared::new(SystemState::build_from_config(&config).expect("state"));

  let props = Props::from_fn(|| NoopActor);
  let pid = state.allocate_pid();
  let cell = ActorCell::create(state.clone(), pid, None, "worker".to_string(), &props).expect("child cell");
  state.register_cell(cell.clone());
  let mut reference = cell.actor_ref();

  let sender_before = sender_calls.load(Ordering::SeqCst);
  let future_before = future_calls.load(Ordering::SeqCst);

  let _response = reference.ask(AnyMessage::new("ask-payload"));

  assert_eq!(
    sender_calls.load(Ordering::SeqCst),
    sender_before + 1,
    "ask reply refs should materialize sender shared handles via the system's configured factory"
  );
  assert_eq!(
    future_calls.load(Ordering::SeqCst),
    future_before + 1,
    "ask futures should materialize via the system's configured factory"
  );
}
