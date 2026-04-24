#![cfg(not(target_os = "none"))]

use std::{
  thread,
  time::{Duration, Instant},
  vec::Vec,
};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::{
  kernel::actor::{Pid, error::ActorError, setup::ActorSystemConfig},
  typed::{
    TypedActorRef, TypedActorSystem, TypedProps,
    actor::{TypedActor, TypedActorContext, TypedChildRef},
    message_adapter::AdapterError,
  },
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

#[derive(Clone, Debug)]
enum RootMsg {
  Start,
  Adapted(u32),
  PipeDone(u32),
  AskReply(u32),
  AskFailed,
}

#[derive(Clone, Debug)]
enum WorkerMsg {
  Ask { reply_to: TypedActorRef<u32> },
}

struct Worker;

impl TypedActor<WorkerMsg> for Worker {
  fn receive(&mut self, _ctx: &mut TypedActorContext<'_, WorkerMsg>, message: &WorkerMsg) -> Result<(), ActorError> {
    match message {
      | WorkerMsg::Ask { reply_to } => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(31);
      },
    }
    Ok(())
  }
}

struct Root {
  adapted_log:    ArcShared<SpinSyncMutex<Vec<u32>>>,
  pipe_log:       ArcShared<SpinSyncMutex<Vec<u32>>>,
  ask_log:        ArcShared<SpinSyncMutex<Vec<u32>>>,
  terminated_log: ArcShared<SpinSyncMutex<Vec<u64>>>,
  child:          Option<TypedChildRef<WorkerMsg>>,
}

impl Root {
  fn new(
    adapted_log: ArcShared<SpinSyncMutex<Vec<u32>>>,
    pipe_log: ArcShared<SpinSyncMutex<Vec<u32>>>,
    ask_log: ArcShared<SpinSyncMutex<Vec<u32>>>,
    terminated_log: ArcShared<SpinSyncMutex<Vec<u64>>>,
  ) -> Self {
    Self { adapted_log, pipe_log, ask_log, terminated_log, child: None }
  }
}

impl TypedActor<RootMsg> for Root {
  fn receive(&mut self, ctx: &mut TypedActorContext<'_, RootMsg>, message: &RootMsg) -> Result<(), ActorError> {
    match message {
      | RootMsg::Start => {
        let props = TypedProps::<WorkerMsg>::new(|| Worker).map_props(|props| props.with_name("typed-e2e-worker"));
        let child = ctx
          .spawn_child(&props)
          .map_err(|error| ActorError::recoverable(format!("typed E2E spawn failed: {error:?}")))?;
        ctx
          .watch(&child.actor_ref())
          .map_err(|error| ActorError::recoverable(format!("typed E2E watch failed: {error:?}")))?;

        let mut adapter = ctx
          .message_adapter(|value: u32| Ok(RootMsg::Adapted(value)))
          .map_err(|error| ActorError::recoverable(format!("typed E2E adapter failed: {error:?}")))?;
        adapter.tell(5);

        ctx
          .pipe_to_self(
            async { Ok::<u32, ()>(11) },
            |value| Ok(RootMsg::PipeDone(value)),
            |_error| Ok(RootMsg::PipeDone(0)),
          )
          .map_err(|error| ActorError::recoverable(format!("typed E2E pipeToSelf failed: {error:?}")))?;

        let mut child_ref = child.actor_ref();
        ctx
          .ask(
            &mut child_ref,
            |reply_to| WorkerMsg::Ask { reply_to },
            |result| match result {
              | Ok(value) => RootMsg::AskReply(value),
              | Err(_) => RootMsg::AskFailed,
            },
            Duration::from_secs(5),
          )
          .map_err(|error| ActorError::recoverable(format!("typed E2E ask failed: {error:?}")))?;
        self.child = Some(child);
      },
      | RootMsg::Adapted(value) => self.adapted_log.lock().push(*value),
      | RootMsg::PipeDone(value) => self.pipe_log.lock().push(*value),
      | RootMsg::AskReply(value) => {
        self.ask_log.lock().push(*value);
        if let Some(child) = &self.child {
          child.stop().map_err(|error| ActorError::recoverable(format!("typed E2E stop failed: {error:?}")))?;
        }
      },
      | RootMsg::AskFailed => return Err(ActorError::recoverable("typed E2E ask failed")),
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut TypedActorContext<'_, RootMsg>, terminated: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(terminated.value());
    Ok(())
  }

  fn on_adapter_failure(
    &mut self,
    _ctx: &mut TypedActorContext<'_, RootMsg>,
    error: AdapterError,
  ) -> Result<(), ActorError> {
    Err(ActorError::recoverable(format!("typed E2E adapter delivery failed: {error:?}")))
  }
}

#[test]
fn typed_user_flow_observes_spawn_adapter_ask_pipe_stop_and_signal() {
  let adapted_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let pipe_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ask_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let terminated_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = TypedProps::<RootMsg>::new({
    let adapted_log = adapted_log.clone();
    let pipe_log = pipe_log.clone();
    let ask_log = ask_log.clone();
    let terminated_log = terminated_log.clone();
    move || Root::new(adapted_log.clone(), pipe_log.clone(), ask_log.clone(), terminated_log.clone())
  });
  let system =
    TypedActorSystem::<RootMsg>::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("typed system");
  let mut guardian = system.user_guardian_ref();

  guardian.tell(RootMsg::Start);

  assert!(wait_until(200, || {
    *adapted_log.lock() == vec![5]
      && *pipe_log.lock() == vec![11]
      && *ask_log.lock() == vec![31]
      && terminated_log.lock().len() == 1
  }));

  assert_eq!(*adapted_log.lock(), vec![5]);
  assert_eq!(*pipe_log.lock(), vec![11]);
  assert_eq!(*ask_log.lock(), vec![31]);
  assert_eq!(terminated_log.lock().len(), 1);
  system.terminate().expect("terminate");
}

fn wait_until(deadline_ms: u64, mut predicate: impl FnMut() -> bool) -> bool {
  let deadline = Instant::now() + Duration::from_millis(deadline_ms);
  while Instant::now() < deadline {
    if predicate() {
      return true;
    }
    thread::yield_now();
  }
  predicate()
}
