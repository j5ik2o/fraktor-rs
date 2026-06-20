use alloc::boxed::Box;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::actor::{
  actor_cell::tests::*,
  actor_cell_dispatch::ActorCellInvoker,
  context_pipe::ContextPipeTaskId,
  messaging::{message_invoker::MessageInvoker, system_message::SystemMessage},
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
