use core::time::Duration;

use crate::core::{
  actor::actor_ref::ActorRef,
  scheduler::{Scheduler, SchedulerCommand, SchedulerConfig},
  typed::{
    actor::TypedActorRef,
    scheduler::{TypedScheduler, TypedSchedulerContext},
  },
};

fn build_scheduler() -> Scheduler {
  let config = SchedulerConfig::default();
  Scheduler::new(config)
}

#[test]
fn typed_schedule_once_forwards_sender_metadata() {
  let mut scheduler = build_scheduler();
  {
    let mut typed_scheduler = TypedScheduler::new(&mut scheduler);
    let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
    let sender = TypedActorRef::<u32>::from_untyped(ActorRef::null());
    let handle = typed_scheduler
      .schedule_once(Duration::from_millis(1), receiver.clone(), 7u32, None, Some(sender.clone()))
      .expect("handle");
    match scheduler.command_for_test(&handle) {
      | Some(SchedulerCommand::SendMessage { sender: stored_sender, .. }) => {
        assert!(stored_sender.is_some());
      },
      | other => panic!("unexpected command: {:?}", other),
    }
  }
}

#[test]
fn typed_schedule_at_fixed_rate_registers_job() {
  let mut scheduler = build_scheduler();
  {
    let mut typed_scheduler = TypedScheduler::new(&mut scheduler);
    let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
    let handle = typed_scheduler
      .schedule_at_fixed_rate(Duration::from_millis(2), Duration::from_millis(3), receiver.clone(), 3u32, None, None)
      .expect("handle");
    assert!(scheduler.command_for_test(&handle).is_some());
  }
}

#[test]
fn typed_scheduler_context_reuses_shared_scheduler_arc() {
  let context = TypedSchedulerContext::new_with_config(SchedulerConfig::default());
  let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());

  let handle = context.with_scheduler(|scheduler| {
    scheduler.schedule_once(Duration::from_millis(5), receiver.clone(), 99u32, None, None).expect("handle")
  });

  let scheduler_arc = context.scheduler();
  {
    scheduler_arc.with_write(|guard| {
      assert!(guard.command_for_test(&handle).is_some());
    });
  }
}
