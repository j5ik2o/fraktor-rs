use fraktor_actor_core_kernel_rs::actor::ActorContext;

use crate::{
  actor::TypedActorContext, behavior_interceptor::BehaviorInterceptor, dsl::Behaviors,
  message_and_signals::BehaviorSignal,
};

#[test]
fn interceptor_trait_has_default_around_start() {
  struct NoopInterceptor;
  impl BehaviorInterceptor<u32> for NoopInterceptor {}

  let mut interceptor = NoopInterceptor;
  let system = fraktor_actor_adaptor_std_rs::system::new_empty_actor_system();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let result = interceptor.around_start(&mut typed_ctx, &mut |_ctx| Ok(Behaviors::same()));
  assert!(result.is_ok());
}

#[test]
fn interceptor_trait_has_default_around_receive() {
  struct NoopInterceptor;
  impl BehaviorInterceptor<u32> for NoopInterceptor {}

  let mut interceptor = NoopInterceptor;
  let system = fraktor_actor_adaptor_std_rs::system::new_empty_actor_system();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let result = interceptor.around_receive(&mut typed_ctx, &42u32, &mut |_ctx, _msg| Ok(Behaviors::same()));
  assert!(result.is_ok());
}

#[test]
fn interceptor_trait_has_default_around_signal() {
  struct NoopInterceptor;
  impl BehaviorInterceptor<u32> for NoopInterceptor {}

  let mut interceptor = NoopInterceptor;
  let system = fraktor_actor_adaptor_std_rs::system::new_empty_actor_system();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let result =
    interceptor.around_signal(&mut typed_ctx, &BehaviorSignal::PostStop, &mut |_ctx, _sig| Ok(Behaviors::same()));
  assert!(result.is_ok());
}

#[test]
fn interceptor_trait_default_is_same_returns_true_for_same_instance() {
  struct IdentityInterceptor;

  impl BehaviorInterceptor<u32> for IdentityInterceptor {}

  let interceptor = IdentityInterceptor;
  let this: &dyn BehaviorInterceptor<u32> = &interceptor;

  assert!(this.is_same(this));
}

#[test]
fn interceptor_trait_default_is_same_returns_false_for_distinct_instances() {
  struct IdentityInterceptor;

  impl BehaviorInterceptor<u32> for IdentityInterceptor {}

  let left = IdentityInterceptor;
  let right = IdentityInterceptor;
  let left_ref: &dyn BehaviorInterceptor<u32> = &left;
  let right_ref: &dyn BehaviorInterceptor<u32> = &right;

  assert!(!left_ref.is_same(right_ref));
}

#[test]
fn interceptor_trait_is_same_can_be_overridden() {
  struct AlwaysSameInterceptor;

  impl BehaviorInterceptor<u32> for AlwaysSameInterceptor {
    fn is_same(&self, _other: &dyn BehaviorInterceptor<u32>) -> bool {
      true
    }
  }

  let left = AlwaysSameInterceptor;
  let right = AlwaysSameInterceptor;
  let left_ref: &dyn BehaviorInterceptor<u32> = &left;
  let right_ref: &dyn BehaviorInterceptor<u32> = &right;

  assert!(left_ref.is_same(right_ref));
}
