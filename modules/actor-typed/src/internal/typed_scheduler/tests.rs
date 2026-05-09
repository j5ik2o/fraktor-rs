use core::time::Duration;

use fraktor_actor_core_kernel_rs::actor::{
  actor_ref::ActorRef,
  scheduler::{Scheduler, SchedulerConfig, SchedulerContext, SchedulerHandle, SchedulerMode},
};

use crate::{
  TypedActorRef,
  internal::{TypedScheduler, TypedSchedulerShared},
};

fn build_scheduler() -> Scheduler {
  let config = SchedulerConfig::default();
  Scheduler::new(config)
}

fn has_scheduled_job(scheduler: &Scheduler, handle: &SchedulerHandle) -> bool {
  scheduler.dump().jobs().iter().any(|job| job.handle_id() == handle.raw())
}

#[test]
fn typed_schedule_once_registers_one_shot_job() {
  let mut scheduler = build_scheduler();
  {
    let mut typed_scheduler = TypedScheduler::new(&mut scheduler);
    let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
    let sender = TypedActorRef::<u32>::from_untyped(ActorRef::null());
    let handle = typed_scheduler
      .schedule_once(Duration::from_millis(1), receiver.clone(), 7u32, Some(sender.clone()))
      .expect("handle");
    let dump = scheduler.dump();
    assert!(dump.jobs().iter().any(|job| { job.handle_id() == handle.raw() && job.mode() == SchedulerMode::OneShot }));
  }
}

#[test]
fn typed_schedule_at_fixed_rate_registers_job() {
  let mut scheduler = build_scheduler();
  {
    let mut typed_scheduler = TypedScheduler::new(&mut scheduler);
    let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
    let handle = typed_scheduler
      .schedule_at_fixed_rate(Duration::from_millis(2), Duration::from_millis(3), receiver.clone(), 3u32, None)
      .expect("handle");
    assert!(has_scheduled_job(&scheduler, &handle));
  }
}

#[test]
fn typed_scheduler_shared_reuses_scheduler_handle() {
  let context = SchedulerContext::new(SchedulerConfig::default());
  let shared = TypedSchedulerShared::new(context.scheduler());
  let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());

  let handle = shared
    .with_write(|guard| guard.schedule_once(Duration::from_millis(5), receiver.clone(), 99u32, None))
    .expect("handle");

  {
    shared.with_write(|guard| {
      assert!(has_scheduled_job(&guard, &handle));
    });
  }
}
