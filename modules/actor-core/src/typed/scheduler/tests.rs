use core::time::Duration;

use crate::{
  NoStdToolbox,
  actor_prim::actor_ref::ActorRefGeneric,
  scheduler::{Scheduler, SchedulerConfig, command::SchedulerCommand},
  typed::{actor_prim::TypedActorRefGeneric, scheduler::TypedScheduler},
};

fn build_scheduler() -> Scheduler<NoStdToolbox> {
  let toolbox = NoStdToolbox::default();
  let config = SchedulerConfig::default();
  Scheduler::new(toolbox, config)
}

#[test]
fn typed_schedule_once_forwards_sender_metadata() {
  let mut scheduler = build_scheduler();
  {
    let mut typed_scheduler = TypedScheduler::new(&mut scheduler);
    let receiver = TypedActorRefGeneric::<u32, NoStdToolbox>::from_untyped(ActorRefGeneric::null());
    let sender = TypedActorRefGeneric::<u32, NoStdToolbox>::from_untyped(ActorRefGeneric::null());
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
    let receiver = TypedActorRefGeneric::<u32, NoStdToolbox>::from_untyped(ActorRefGeneric::null());
    let handle = typed_scheduler
      .schedule_at_fixed_rate(Duration::from_millis(2), Duration::from_millis(3), receiver.clone(), 3u32, None, None)
      .expect("handle");
    assert!(scheduler.command_for_test(&handle).is_some());
  }
}
