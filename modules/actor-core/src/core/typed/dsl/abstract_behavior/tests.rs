use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::{
  kernel::{
    actor::{ActorContext, error::ActorError},
    system::ActorSystem,
  },
  typed::{
    ExtensibleBehavior,
    actor::TypedActorContext,
    behavior::Behavior,
    dsl::{AbstractBehavior, Behaviors},
    message_and_signals::BehaviorSignal,
  },
};

// --- T8: AbstractBehavior tests ---

/// A counter actor that increments on each message and participates in
/// behavior transitions (returns `Behaviors::same()`).
struct CounterBehavior {
  count: ArcShared<SpinSyncMutex<u32>>,
}

impl AbstractBehavior<u32> for CounterBehavior {
  fn on_message(&mut self, _ctx: &mut TypedActorContext<'_, u32>, msg: &u32) -> Result<Behavior<u32>, ActorError> {
    *self.count.lock() += msg;
    Ok(Behaviors::same())
  }
}

/// An actor that transitions to `stopped` after receiving a specific message.
struct StoppingBehavior;

impl AbstractBehavior<u32> for StoppingBehavior {
  fn on_message(&mut self, _ctx: &mut TypedActorContext<'_, u32>, msg: &u32) -> Result<Behavior<u32>, ActorError> {
    if *msg == 0 { Ok(Behaviors::stopped()) } else { Ok(Behaviors::same()) }
  }
}

/// An actor with a custom signal handler.
struct SignalAwareBehavior {
  signal_received: ArcShared<SpinSyncMutex<bool>>,
}

impl AbstractBehavior<u32> for SignalAwareBehavior {
  fn on_message(&mut self, _ctx: &mut TypedActorContext<'_, u32>, _msg: &u32) -> Result<Behavior<u32>, ActorError> {
    Ok(Behaviors::same())
  }

  fn on_signal(
    &mut self,
    _ctx: &mut TypedActorContext<'_, u32>,
    _signal: &BehaviorSignal,
  ) -> Result<Behavior<u32>, ActorError> {
    *self.signal_received.lock() = true;
    Ok(Behaviors::same())
  }
}

struct ExtensibleCounterBehavior {
  count: ArcShared<SpinSyncMutex<u32>>,
}

impl ExtensibleBehavior<u32> for ExtensibleCounterBehavior {
  fn receive(&mut self, _ctx: &mut TypedActorContext<'_, u32>, msg: &u32) -> Result<Behavior<u32>, ActorError> {
    *self.count.lock() += msg;
    Ok(Behaviors::same())
  }
}

struct ExtensibleSignalAwareBehavior {
  signal_received: ArcShared<SpinSyncMutex<bool>>,
}

impl ExtensibleBehavior<u32> for ExtensibleSignalAwareBehavior {
  fn receive(&mut self, _ctx: &mut TypedActorContext<'_, u32>, _msg: &u32) -> Result<Behavior<u32>, ActorError> {
    Ok(Behaviors::same())
  }

  fn receive_signal(
    &mut self,
    _ctx: &mut TypedActorContext<'_, u32>,
    _signal: &BehaviorSignal,
  ) -> Result<Behavior<u32>, ActorError> {
    *self.signal_received.lock() = true;
    Ok(Behaviors::same())
  }
}

#[test]
fn from_abstract_creates_behavior_that_handles_messages() {
  // Given: an AbstractBehavior implementation that counts messages
  let count = ArcShared::new(SpinSyncMutex::new(0u32));
  let count_clone = count.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  // When: from_abstract creates a Behavior and we handle a message
  let mut behavior = Behaviors::from_abstract(move |_ctx: &mut TypedActorContext<'_, u32>| CounterBehavior {
    count: count_clone.clone(),
  });

  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup should succeed");

  // Then: the message is handled by the AbstractBehavior
  let result = inner.handle_message(&mut typed_ctx, &10u32).expect("message should be handled");
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same),
    "on_message returning Behaviors::same() should yield Same directive"
  );
  assert_eq!(*count.lock(), 10, "counter should have incremented by the message value");
}

#[test]
fn from_abstract_handles_multiple_messages_with_state() {
  // Given: an AbstractBehavior that accumulates state
  let count = ArcShared::new(SpinSyncMutex::new(0u32));
  let count_clone = count.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::from_abstract(move |_ctx: &mut TypedActorContext<'_, u32>| CounterBehavior {
    count: count_clone.clone(),
  });
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup");

  // When: multiple messages are sent
  inner.handle_message(&mut typed_ctx, &1u32).expect("first");
  inner.handle_message(&mut typed_ctx, &2u32).expect("second");
  inner.handle_message(&mut typed_ctx, &3u32).expect("third");

  // Then: state is accumulated across messages
  assert_eq!(*count.lock(), 6, "counter should be 1 + 2 + 3 = 6");
}

#[test]
fn from_abstract_supports_behavior_transition_to_stopped() {
  // Given: an AbstractBehavior that stops on message 0
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::from_abstract(|_ctx: &mut TypedActorContext<'_, u32>| StoppingBehavior);
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup");

  // When: a non-stop message is sent
  let same_result = inner.handle_message(&mut typed_ctx, &42u32).expect("should return same");
  assert!(matches!(same_result.directive(), crate::core::typed::behavior::BehaviorDirective::Same));

  // When: a stop message is sent
  let stopped_result = inner.handle_message(&mut typed_ctx, &0u32).expect("should return stopped");

  // Then: the behavior transitions to stopped
  assert!(matches!(stopped_result.directive(), crate::core::typed::behavior::BehaviorDirective::Stopped));
}

#[test]
fn from_abstract_delegates_signals_to_on_signal() {
  // Given: an AbstractBehavior with a custom on_signal implementation
  let signal_received = ArcShared::new(SpinSyncMutex::new(false));
  let signal_clone = signal_received.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::from_abstract(move |_ctx: &mut TypedActorContext<'_, u32>| SignalAwareBehavior {
    signal_received: signal_clone.clone(),
  });
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup");

  // When: a signal is delivered
  inner.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("signal should be handled");

  // Then: the on_signal handler was invoked
  assert!(*signal_received.lock(), "on_signal should have been called");
}

#[test]
fn from_abstract_default_on_signal_returns_unhandled() {
  // Given: an AbstractBehavior that does NOT override on_signal (uses default)
  let count = ArcShared::new(SpinSyncMutex::new(0u32));
  let count_clone = count.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::from_abstract(move |_ctx: &mut TypedActorContext<'_, u32>| CounterBehavior {
    count: count_clone.clone(),
  });
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup");

  // When: a signal is delivered to an actor with default on_signal
  let result = inner.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("signal");

  // Then: the default on_signal returns Unhandled
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Unhandled),
    "default on_signal should return Unhandled"
  );
}

#[test]
fn from_abstract_factory_receives_context() {
  // Given: a factory that captures the pid from context
  let captured_pid = ArcShared::new(SpinSyncMutex::new(0u64));
  let captured_pid_clone = captured_pid.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let count = ArcShared::new(SpinSyncMutex::new(0u32));
  let mut behavior = Behaviors::from_abstract(move |ctx: &mut TypedActorContext<'_, u32>| {
    *captured_pid_clone.lock() = ctx.pid().value();
    CounterBehavior { count: count.clone() }
  });

  behavior.handle_start(&mut typed_ctx).expect("setup");

  // Then: the factory received the correct context
  assert_eq!(*captured_pid.lock(), typed_ctx.pid().value(), "factory should receive the correct pid");
}

#[test]
fn from_abstract_clone_recreates_behavior_on_started() {
  let count = ArcShared::new(SpinSyncMutex::new(0u32));
  let count_clone = count.clone();
  let factory_calls = ArcShared::new(SpinSyncMutex::new(0u32));
  let factory_calls_clone = factory_calls.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let behavior = Behaviors::from_abstract(move |_ctx: &mut TypedActorContext<'_, u32>| {
    *factory_calls_clone.lock() += 1;
    CounterBehavior { count: count_clone.clone() }
  });

  let mut first = behavior.clone().handle_start(&mut typed_ctx).expect("first setup");
  let mut second = behavior.clone().handle_start(&mut typed_ctx).expect("second setup");

  first.handle_message(&mut typed_ctx, &1u32).expect("first message");
  second.handle_message(&mut typed_ctx, &2u32).expect("second message");

  assert_eq!(*factory_calls.lock(), 2, "factory should run for each cloned behavior start");
  assert_eq!(*count.lock(), 3, "each clone should initialize its own abstract behavior instance");
}

#[test]
fn from_extensible_creates_behavior_that_handles_messages() {
  // Given: an ExtensibleBehavior implementation that counts messages
  let count = ArcShared::new(SpinSyncMutex::new(0u32));
  let count_clone = count.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::from_extensible(move |_ctx: &mut TypedActorContext<'_, u32>| {
    ExtensibleCounterBehavior { count: count_clone.clone() }
  });
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup should succeed");

  // When: a message is handled through the factory-created behavior
  let result = inner.handle_message(&mut typed_ctx, &10u32).expect("message should be handled");

  // Then: receive() is delegated and returns Same
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same),
    "receive returning Behaviors::same() should yield Same directive"
  );
  assert_eq!(*count.lock(), 10, "counter should have incremented by the message value");
}

#[test]
fn from_extensible_delegates_signals_to_receive_signal() {
  // Given: an ExtensibleBehavior with a custom receive_signal implementation
  let signal_received = ArcShared::new(SpinSyncMutex::new(false));
  let signal_clone = signal_received.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::from_extensible(move |_ctx: &mut TypedActorContext<'_, u32>| {
    ExtensibleSignalAwareBehavior { signal_received: signal_clone.clone() }
  });
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup should succeed");

  // When: a signal is delivered
  inner.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("signal should be handled");

  // Then: receive_signal() is invoked
  assert!(*signal_received.lock(), "receive_signal should have been called");
}

#[test]
fn from_extensible_default_receive_signal_returns_unhandled() {
  // Given: an ExtensibleBehavior that relies on the default receive_signal implementation
  let count = ArcShared::new(SpinSyncMutex::new(0u32));
  let count_clone = count.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::from_extensible(move |_ctx: &mut TypedActorContext<'_, u32>| {
    ExtensibleCounterBehavior { count: count_clone.clone() }
  });
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("setup should succeed");

  // When: a signal is delivered without an override
  let result = inner.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("signal should be accepted");

  // Then: the default receive_signal returns Unhandled
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Unhandled),
    "default receive_signal should return Unhandled"
  );
}

#[test]
fn from_extensible_coexists_with_from_abstract() {
  // Given: extensible と abstract の factory が同じテスト内で共存する
  let extensible_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let abstract_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let extensible_count_clone = extensible_count.clone();
  let abstract_count_clone = abstract_count.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut extensible = Behaviors::from_extensible(move |_ctx: &mut TypedActorContext<'_, u32>| {
    ExtensibleCounterBehavior { count: extensible_count_clone.clone() }
  });
  let mut abstract_behavior = Behaviors::from_abstract(move |_ctx: &mut TypedActorContext<'_, u32>| CounterBehavior {
    count: abstract_count_clone.clone(),
  });

  let mut extensible_inner = extensible.handle_start(&mut typed_ctx).expect("extensible setup");
  let mut abstract_inner = abstract_behavior.handle_start(&mut typed_ctx).expect("abstract setup");

  // When: both factories handle messages independently
  extensible_inner.handle_message(&mut typed_ctx, &4u32).expect("extensible message");
  abstract_inner.handle_message(&mut typed_ctx, &7u32).expect("abstract message");

  // Then: the two APIs coexist without interfering with each other
  assert_eq!(*extensible_count.lock(), 4, "extensible behavior should keep its own state");
  assert_eq!(*abstract_count.lock(), 7, "abstract behavior should keep its own state");
}
