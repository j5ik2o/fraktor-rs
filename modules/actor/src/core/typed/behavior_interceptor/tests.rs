use crate::core::typed::{
  actor::TypedActorContext, behavior_interceptor::BehaviorInterceptor, behavior_signal::BehaviorSignal, dsl::Behaviors,
};

#[test]
fn interceptor_trait_has_default_around_start() {
  struct NoopInterceptor;
  impl BehaviorInterceptor<u32> for NoopInterceptor {}

  let mut interceptor = NoopInterceptor;
  let system = crate::core::kernel::system::ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = crate::core::kernel::actor::ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let result = interceptor.around_start(&mut typed_ctx, &mut |_ctx| Ok(Behaviors::same()));
  assert!(result.is_ok());
}

#[test]
fn interceptor_trait_has_default_around_receive() {
  struct NoopInterceptor;
  impl BehaviorInterceptor<u32> for NoopInterceptor {}

  let mut interceptor = NoopInterceptor;
  let system = crate::core::kernel::system::ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = crate::core::kernel::actor::ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let result = interceptor.around_receive(&mut typed_ctx, &42u32, &mut |_ctx, _msg| Ok(Behaviors::same()));
  assert!(result.is_ok());
}

#[test]
fn interceptor_trait_has_default_around_signal() {
  struct NoopInterceptor;
  impl BehaviorInterceptor<u32> for NoopInterceptor {}

  let mut interceptor = NoopInterceptor;
  let system = crate::core::kernel::system::ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = crate::core::kernel::actor::ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let result =
    interceptor.around_signal(&mut typed_ctx, &BehaviorSignal::Started, &mut |_ctx, _sig| Ok(Behaviors::same()));
  assert!(result.is_ok());
}
