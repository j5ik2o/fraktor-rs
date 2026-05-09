use alloc::boxed::Box;

use fraktor_actor_core_kernel_rs::actor::ActorContext;
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::BehaviorSignalInterceptor;
use crate::{
  Behavior, actor::TypedActorContext, dsl::Behaviors, internal::behavior_signal_interceptor::ActorError,
  message_and_signals::BehaviorSignal,
};

struct SignalProbe {
  start_count:  ArcShared<SpinSyncMutex<u32>>,
  signal_count: ArcShared<SpinSyncMutex<u32>>,
}

impl BehaviorSignalInterceptor<u32> for SignalProbe {
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, u32>,
    start: &mut (dyn FnMut(&mut TypedActorContext<'_, u32>) -> Result<Behavior<u32>, ActorError> + '_),
  ) -> Result<Behavior<u32>, ActorError> {
    *self.start_count.lock() += 1;
    start(ctx)
  }

  fn around_signal(
    &mut self,
    ctx: &mut TypedActorContext<'_, u32>,
    signal: &BehaviorSignal,
    target: &mut (
           dyn FnMut(&mut TypedActorContext<'_, u32>, &BehaviorSignal) -> Result<Behavior<u32>, ActorError> + '_
         ),
  ) -> Result<Behavior<u32>, ActorError> {
    *self.signal_count.lock() += 1;
    target(ctx, signal)
  }
}

#[test]
fn behavior_signal_interceptor_default_handlers_delegate() {
  let start_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let signal_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let mut interceptor = SignalProbe { start_count: start_count.clone(), signal_count: signal_count.clone() };
  let system = fraktor_actor_adaptor_std_rs::system::new_empty_actor_system();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut start = |_ctx: &mut TypedActorContext<'_, u32>| Ok(Behaviors::ignore());
  let _started_behavior = interceptor.around_start(&mut typed_ctx, &mut start).expect("started");

  let mut signal_target = |_ctx: &mut TypedActorContext<'_, u32>, _signal: &BehaviorSignal| Ok(Behaviors::same());
  let _signal_behavior =
    interceptor.around_signal(&mut typed_ctx, &BehaviorSignal::PostStop, &mut signal_target).expect("post stop");

  assert_eq!(*start_count.lock(), 1);
  assert_eq!(*signal_count.lock(), 1);
}

#[test]
fn intercept_signal_delegates_to_signal_interceptor() {
  let start_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let signal_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let start_count_clone = start_count.clone();
  let signal_count_clone = signal_count.clone();

  let mut behavior = Behaviors::intercept_signal(
    move || Box::new(SignalProbe { start_count: start_count_clone.clone(), signal_count: signal_count_clone.clone() }),
    || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())),
  );

  let system = fraktor_actor_adaptor_std_rs::system::new_empty_actor_system();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut started = behavior.handle_start(&mut typed_ctx).expect("started");
  started.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("post stop");

  assert_eq!(*start_count.lock(), 1);
  assert_eq!(*signal_count.lock(), 1);
}
