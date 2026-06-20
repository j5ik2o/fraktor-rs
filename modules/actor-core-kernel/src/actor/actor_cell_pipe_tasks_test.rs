use alloc::boxed::Box;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::{
  actor::{
    actor_cell::tests::*,
    actor_cell_dispatch::ActorCellInvoker,
    context_pipe::ContextPipeTaskId,
    error::SendError,
    messaging::{message_invoker::MessageInvoker, system_message::SystemMessage},
  },
  dispatch::{
    dispatcher::{
      DispatcherConfig, DispatcherCore, ExecutorShared, InlineExecutor, MessageDispatcher, MessageDispatcherFactory,
      MessageDispatcherShared, TrampolineState,
    },
    mailbox::Mailbox,
  },
};

struct ReentrantPipeFuture {
  cell:    ArcShared<ActorCell>,
  task_id: ContextPipeTaskId,
  polls:   ArcShared<SpinSyncMutex<usize>>,
}

impl Future for ReentrantPipeFuture {
  type Output = Option<AnyMessage>;

  fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
    let mut polls = self.polls.lock();
    *polls += 1;
    let current_poll = *polls;
    drop(polls);

    if current_poll == 1 {
      self.cell.handle_pipe_task_ready(self.task_id);
      return Poll::Pending;
    }

    Poll::Ready(None)
  }
}

struct FailingSystemDispatcherFactory;

impl MessageDispatcherFactory for FailingSystemDispatcherFactory {
  fn dispatcher(&self) -> MessageDispatcherShared {
    let throughput = NonZeroUsize::new(1).expect("throughput");
    let settings = DispatcherConfig::new("pipe-repoll-failing", throughput, None, Duration::from_secs(1));
    let executor = ExecutorShared::new(Box::new(InlineExecutor::new()), TrampolineState::new());
    MessageDispatcherShared::new(Box::new(FailingSystemDispatcher { core: DispatcherCore::new(&settings, executor) }))
  }
}

struct FailingSystemDispatcher {
  core: DispatcherCore,
}

impl MessageDispatcher for FailingSystemDispatcher {
  fn core(&self) -> &DispatcherCore {
    &self.core
  }

  fn core_mut(&mut self) -> &mut DispatcherCore {
    &mut self.core
  }

  fn system_dispatch(
    &mut self,
    _receiver: &ArcShared<ActorCell>,
    message: SystemMessage,
  ) -> Result<Vec<ArcShared<Mailbox>>, SendError> {
    Err(SendError::closed(AnyMessage::new(message)))
  }
}

fn failing_system_dispatcher_factory() -> ArcShared<Box<dyn MessageDispatcherFactory>> {
  ArcShared::new(Box::new(FailingSystemDispatcherFactory) as Box<dyn MessageDispatcherFactory>)
}

#[test]
fn spawn_pipe_task_rejects_terminated_cell() {
  let actor_system = ActorSystem::new_empty();
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(913, 0), None, "pipe-stopped".to_string(), &props).expect("cell");
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  invoker.system_invoke(SystemMessage::Stop).expect("stop");

  let result = cell.spawn_pipe_task(Box::pin(async { Some(AnyMessage::new(1_i32)) }));

  assert!(matches!(result, Err(PipeSpawnError::TargetStopped)));
}

#[test]
fn spawn_pipe_task_records_self_delivery_error_when_mailbox_is_closed() {
  let actor_system = ActorSystem::new_empty();
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(system.clone(), Pid::new(914, 0), None, "pipe-self-closed".to_string(), &props).expect("cell");
  system.register_cell(cell.clone());
  cell.mailbox().become_closed();

  cell.spawn_pipe_task(Box::pin(async { Some(AnyMessage::new(1_i32)) })).expect("spawn pipe task");

  assert!(
    system.dead_letters().iter().any(|entry| entry.recipient() == Some(cell.pid())),
    "pipe_to_self delivery failure should be recorded"
  );
}

#[test]
fn poll_pipe_task_preserves_reentrant_wake_while_future_is_polling() {
  let actor_system = ActorSystem::new_empty();
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(system.clone(), Pid::new(915, 0), None, "pipe-reentrant".to_string(), &props).expect("cell");
  system.register_cell(cell.clone());

  let polls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let task_id = ContextPipeTaskId::new(1);
  cell
    .spawn_pipe_task(Box::pin(ReentrantPipeFuture { cell: cell.clone(), task_id, polls: polls.clone() }))
    .expect("spawn pipe task");

  assert_eq!(*polls.lock(), 2, "reentrant wake should be replayed after the pending poll completes");
}

#[test]
fn poll_pipe_task_records_repoll_send_error_when_dispatcher_rejects() {
  let actor_system = ActorSystem::new_empty_with(|config| {
    config.with_dispatcher_factory("pipe-repoll-failing", failing_system_dispatcher_factory())
  });
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor).with_dispatcher_id("pipe-repoll-failing");
  let cell =
    ActorCell::create(system.clone(), Pid::new(916, 0), None, "pipe-repoll-failing".to_string(), &props).expect("cell");
  system.register_cell(cell.clone());

  let polls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let task_id = ContextPipeTaskId::new(1);
  cell
    .spawn_pipe_task(Box::pin(ReentrantPipeFuture { cell: cell.clone(), task_id, polls: polls.clone() }))
    .expect("spawn pipe task");

  assert_eq!(*polls.lock(), 1, "failing dispatcher should reject the scheduled re-poll");
  assert!(
    system.dead_letters().iter().any(|entry| {
      entry.recipient() == Some(cell.pid())
        && entry.message().downcast_ref::<SystemMessage>().is_some_and(|message| {
          matches!(
            message,
            SystemMessage::PipeTask(id) if *id == task_id
          )
        })
    }),
    "failed PipeTask re-poll scheduling should be recorded"
  );
}
