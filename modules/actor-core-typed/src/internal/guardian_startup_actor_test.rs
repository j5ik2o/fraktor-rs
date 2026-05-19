use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use fraktor_actor_core_kernel_rs::actor::{
  Actor, ActorContext,
  error::{ActorError, ActorErrorReason},
  messaging::{AnyMessage, AnyMessageView},
};

use super::{
  GUARDIAN_STARTUP_DEFERRED_LIMIT, GUARDIAN_STARTUP_DEFERRED_NO_ACTIVE_MESSAGE_REASON,
  GUARDIAN_STARTUP_DEFERRED_OVERFLOW_REASON, GuardianStartupActor, GuardianStartupDeferralError, GuardianStartupStart,
};

struct SilentActor;

impl Actor for SilentActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct LifecycleProbeActor {
  pre_start_calls:    Arc<AtomicUsize>,
  post_restart_calls: Arc<AtomicUsize>,
}

impl Actor for LifecycleProbeActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.pre_start_calls.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }

  fn post_restart(&mut self, _ctx: &mut ActorContext<'_>, _reason: &ActorErrorReason) -> Result<(), ActorError> {
    self.post_restart_calls.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_context() -> ActorContext<'static> {
  let system = fraktor_actor_adaptor_std_rs::system::create_noop_actor_system();
  let pid = system.allocate_pid();
  ActorContext::new(&system, pid)
}

fn receive_user_message(
  actor: &mut GuardianStartupActor,
  ctx: &mut ActorContext<'_>,
  value: usize,
) -> Result<(), ActorError> {
  ctx.with_current_message(AnyMessage::new(value), |active_ctx, current_message| {
    actor.receive(active_ctx, current_message.as_view())
  })
}

#[test]
fn deferral_requires_active_user_message_with_recoverable_error() {
  let mut actor = GuardianStartupActor::new(Box::new(SilentActor));
  let mut ctx = build_context();
  let message = AnyMessage::new(0_usize);

  let error = actor.receive(&mut ctx, message.as_view()).expect_err("deferral requires current message in context");

  assert!(matches!(
    error,
    ActorError::Recoverable(reason)
      if reason.as_str() == GUARDIAN_STARTUP_DEFERRED_NO_ACTIVE_MESSAGE_REASON
        && reason.is_source_type::<GuardianStartupDeferralError>()
  ));
}

#[test]
fn deferred_overflow_is_recoverable_before_start() {
  let mut actor = GuardianStartupActor::new(Box::new(SilentActor));
  let mut ctx = build_context();

  for value in 0..GUARDIAN_STARTUP_DEFERRED_LIMIT {
    receive_user_message(&mut actor, &mut ctx, value).expect("message should fit deferred buffer");
  }

  let error = receive_user_message(&mut actor, &mut ctx, GUARDIAN_STARTUP_DEFERRED_LIMIT)
    .expect_err("overflow must fail before guardian startup");
  assert!(matches!(
    error,
    ActorError::Recoverable(reason)
      if reason.as_str() == GUARDIAN_STARTUP_DEFERRED_OVERFLOW_REASON
        && reason.is_source_type::<GuardianStartupDeferralError>()
  ));
}

#[test]
fn startup_deferral_restart_keeps_inner_unstarted_until_start_signal() {
  let pre_start_calls = Arc::new(AtomicUsize::new(0));
  let post_restart_calls = Arc::new(AtomicUsize::new(0));
  let mut actor = GuardianStartupActor::new(Box::new(LifecycleProbeActor {
    pre_start_calls:    pre_start_calls.clone(),
    post_restart_calls: post_restart_calls.clone(),
  }));
  let mut ctx = build_context();
  let error = ActorError::recoverable_typed::<GuardianStartupDeferralError>(GUARDIAN_STARTUP_DEFERRED_OVERFLOW_REASON);

  actor.post_restart(&mut ctx, error.reason()).expect("startup deferral restart should not fail");

  assert!(!actor.started);
  assert_eq!(pre_start_calls.load(Ordering::SeqCst), 0);
  assert_eq!(post_restart_calls.load(Ordering::SeqCst), 0);

  ctx
    .with_current_message(AnyMessage::new(GuardianStartupStart), |active_ctx, current_message| {
      actor.receive(active_ctx, current_message.as_view())
    })
    .expect("guardian startup should start inner actor after deferral restart");

  assert!(actor.started);
  assert_eq!(pre_start_calls.load(Ordering::SeqCst), 1);
  assert_eq!(post_restart_calls.load(Ordering::SeqCst), 0);
}
