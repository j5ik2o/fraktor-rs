use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::{
  kernel::{actor::ActorContext, system::ActorSystem},
  typed::{
    actor::TypedActorContext,
    behavior::{Behavior, BehaviorDirective},
    behavior_signal::BehaviorSignal,
    dsl::Behaviors,
  },
};

// --- helpers ---------------------------------------------------------------

/// Outer message type that wraps an inner `u32`.
#[derive(Clone)]
struct Wrapper(u32);

impl From<Wrapper> for u32 {
  fn from(w: Wrapper) -> Self {
    w.0
  }
}

/// An enum used to test partial matching with `transform_messages`.
#[derive(Clone)]
enum Outer {
  Num(u32),
  Text(()),
}

fn make_ctx(system: &ActorSystem) -> (crate::core::kernel::actor::Pid, ActorContext<'_>) {
  let pid = system.allocate_pid();
  let context = ActorContext::new(system, pid);
  (pid, context)
}

// --- transform_messages: conversion success --------------------------------

#[test]
fn transform_messages_forwards_matched_message_to_inner() {
  let received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let received_clone = received.clone();

  let inner: Behavior<u32> = Behaviors::receive_message(move |_ctx, msg: &u32| {
    received_clone.lock().push(*msg);
    Ok(Behaviors::same())
  });

  let mut outer: Behavior<Outer> = inner.transform_messages(|msg: &Outer| match msg {
    | Outer::Num(n) => Some(*n),
    | Outer::Text(_) => None,
  });

  let system = ActorSystem::new_empty();
  let (_pid, mut context) = make_ctx(&system);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  // Initialise via Started signal.
  let mut active = outer.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  // Send a matching message.
  let result = active.handle_message(&mut typed_ctx, &Outer::Num(42)).expect("message");
  assert!(matches!(result.directive(), BehaviorDirective::Same));

  assert_eq!(received.lock().as_slice(), &[42]);
}

// --- transform_messages: conversion failure → unhandled --------------------

#[test]
fn transform_messages_returns_unhandled_for_non_matching_message() {
  let inner: Behavior<u32> = Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same()));

  let mut outer: Behavior<Outer> = inner.transform_messages(|msg: &Outer| match msg {
    | Outer::Num(n) => Some(*n),
    | Outer::Text(_) => None,
  });

  let system = ActorSystem::new_empty();
  let (_pid, mut context) = make_ctx(&system);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut active = outer.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  let result = active.handle_message(&mut typed_ctx, &Outer::Text(())).expect("unhandled");
  assert!(matches!(result.directive(), BehaviorDirective::Unhandled));
}

// --- transform_messages: signals pass through ------------------------------

#[test]
fn transform_messages_forwards_signals_to_inner() {
  let signal_received = ArcShared::new(NoStdMutex::new(false));
  let signal_clone = signal_received.clone();

  let inner: Behavior<u32> =
    Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())).receive_signal(move |_ctx, signal| {
      if matches!(signal, BehaviorSignal::Stopped) {
        *signal_clone.lock() = true;
      }
      Ok(Behaviors::same())
    });

  let mut outer: Behavior<Outer> = inner.transform_messages(|msg: &Outer| match msg {
    | Outer::Num(n) => Some(*n),
    | Outer::Text(_) => None,
  });

  let system = ActorSystem::new_empty();
  let (_pid, mut context) = make_ctx(&system);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut active = outer.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  let result = active.handle_signal(&mut typed_ctx, &BehaviorSignal::Stopped).expect("signal");
  assert!(matches!(result.directive(), BehaviorDirective::Same));
  assert!(*signal_received.lock());
}

// --- transform_messages: inner stops → outer stops -------------------------

#[test]
fn transform_messages_propagates_stopped_from_inner() {
  let inner: Behavior<u32> = Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::stopped()));

  let mut outer: Behavior<Outer> = inner.transform_messages(|msg: &Outer| match msg {
    | Outer::Num(n) => Some(*n),
    | Outer::Text(_) => None,
  });

  let system = ActorSystem::new_empty();
  let (_pid, mut context) = make_ctx(&system);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut active = outer.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  let result = active.handle_message(&mut typed_ctx, &Outer::Num(1)).expect("stopped");
  assert!(matches!(result.directive(), BehaviorDirective::Stopped));
}

// --- narrow: Into-based type narrowing -------------------------------------

#[test]
fn narrow_converts_via_into() {
  let received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let received_clone = received.clone();

  let inner: Behavior<u32> = Behaviors::receive_message(move |_ctx, msg: &u32| {
    received_clone.lock().push(*msg);
    Ok(Behaviors::same())
  });

  let mut outer: Behavior<Wrapper> = inner.narrow();

  let system = ActorSystem::new_empty();
  let (_pid, mut context) = make_ctx(&system);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut active = outer.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  let result = active.handle_message(&mut typed_ctx, &Wrapper(99)).expect("message");
  assert!(matches!(result.directive(), BehaviorDirective::Same));

  assert_eq!(received.lock().as_slice(), &[99]);
}

// --- Behaviors::transform_messages factory ---------------------------------

#[test]
fn behaviors_transform_messages_delegates_to_behavior_method() {
  let received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let received_clone = received.clone();

  let inner: Behavior<u32> = Behaviors::receive_message(move |_ctx, msg: &u32| {
    received_clone.lock().push(*msg);
    Ok(Behaviors::same())
  });

  let mut outer: Behavior<Outer> = Behaviors::transform_messages(inner, |msg: &Outer| match msg {
    | Outer::Num(n) => Some(*n),
    | Outer::Text(_) => None,
  });

  let system = ActorSystem::new_empty();
  let (_pid, mut context) = make_ctx(&system);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut active = outer.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
  let result = active.handle_message(&mut typed_ctx, &Outer::Num(7)).expect("message");
  assert!(matches!(result.directive(), BehaviorDirective::Same));

  assert_eq!(received.lock().as_slice(), &[7]);
}

// --- transform_messages: inner behavior evolves ----------------------------

#[test]
fn transform_messages_inner_behavior_evolves_on_active() {
  let call_count = ArcShared::new(NoStdMutex::new(0u32));
  let count_clone = call_count.clone();

  // Inner behavior returns a new active behavior on first message.
  let inner: Behavior<u32> = Behaviors::receive_message(move |_ctx, _msg: &u32| {
    let count = count_clone.clone();
    Ok(Behaviors::receive_message(move |_ctx, msg: &u32| {
      *count.lock() += *msg;
      Ok(Behaviors::same())
    }))
  });

  let mut outer: Behavior<Outer> = inner.transform_messages(|msg: &Outer| match msg {
    | Outer::Num(n) => Some(*n),
    | Outer::Text(_) => None,
  });

  let system = ActorSystem::new_empty();
  let (_pid, mut context) = make_ctx(&system);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut active = outer.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  // First message triggers behavior evolution.
  active.handle_message(&mut typed_ctx, &Outer::Num(0)).expect("evolve");

  // Second message goes to the evolved inner behavior.
  active.handle_message(&mut typed_ctx, &Outer::Num(10)).expect("second");

  assert_eq!(*call_count.lock(), 10);
}
