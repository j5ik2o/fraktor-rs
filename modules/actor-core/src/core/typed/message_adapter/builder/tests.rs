use alloc::string::ToString;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::{
  kernel::{
    actor::{Actor, ActorCell, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props},
    system::ActorSystem,
  },
  typed::{
    actor::TypedActorContext,
    message_adapter::{AdapterError, AdapterOutcome, AdapterPayload, MessageAdapterRegistry},
  },
};

struct Harness {
  system: ActorSystem,
  cell:   ArcShared<ActorCell>,
}

impl Harness {
  fn new() -> Self {
    let system = ActorSystem::new_empty();
    let state = system.state();
    let props = Props::from_fn(|| ProbeActor);
    let pid = state.allocate_pid();
    let cell = ActorCell::create(state.clone(), pid, None, "adapter-builder".to_string(), &props).expect("create cell");
    state.register_cell(cell.clone());
    Self { system, cell }
  }

  fn context(&self) -> ActorContext<'_> {
    ActorContext::new(&self.system, self.cell.pid())
  }
}

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn register_map_installs_adapter() {
  let harness = Harness::new();
  let mut context = harness.context();
  let mut registry = MessageAdapterRegistry::<i32>::new();

  {
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, Some(&mut registry));
    let _adapter = typed_ctx.message_adapter_builder::<u32>().register_map(|value| value as i32).expect("register");
  }

  let payload = AdapterPayload::new(7_u32);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::Converted(7));
  assert!(leftover.is_none());
}

#[test]
fn register_with_name_installs_adapter() {
  let harness = Harness::new();
  let mut context = harness.context();
  let mut registry = MessageAdapterRegistry::<i32>::new();

  {
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, Some(&mut registry));
    let _adapter = typed_ctx
      .message_adapter_builder::<u32>()
      .with_name("counter-input")
      .register(|value| Ok((value as i32) * 2))
      .expect("register");
  }

  let payload = AdapterPayload::new(7_u32);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::Converted(14));
  assert!(leftover.is_none());
}

#[test]
fn register_fails_when_registry_is_unavailable() {
  let harness = Harness::new();
  let mut context = harness.context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let result = typed_ctx.message_adapter_builder::<u32>().register_map(|value| value as i32);
  assert!(matches!(result, Err(AdapterError::RegistryUnavailable)));
}
