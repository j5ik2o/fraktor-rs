use alloc::string::ToString;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::{
  kernel::{
    actor::{Actor, ActorCell, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props},
    system::ActorSystem,
  },
  typed::message_adapter::{AdapterError, AdapterOutcome, AdapterPayload, MessageAdapterRegistry},
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
    let cell = ActorCell::create(state.clone(), pid, None, "adapter".to_string(), &props).expect("create actor cell");
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
fn registry_replaces_existing_adapter_for_same_type() {
  let harness = Harness::new();
  let mut registry = MessageAdapterRegistry::<i32>::new();
  let ctx = harness.context();

  registry.register::<u32, _>(&ctx, |value| Ok(value as i32)).expect("first adapter");
  registry.register::<u32, _>(&ctx, |value| Ok((value as i32) * 2)).expect("second adapter");

  assert_eq!(registry.len(), 1);

  let payload = AdapterPayload::new(5_u32);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::Converted(10));
  assert!(leftover.is_none());
}

#[test]
fn registry_returns_not_found_when_no_adapter_matches() {
  let registry = MessageAdapterRegistry::<i32>::new();
  let payload = AdapterPayload::new(1_u8);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::NotFound);
  assert!(leftover.is_some());
}

#[test]
fn registry_returns_failure_from_adapter() {
  let harness = Harness::new();
  let mut registry = MessageAdapterRegistry::<i32>::new();
  let ctx = harness.context();
  registry.register::<u32, _>(&ctx, |_| Err(AdapterError::Custom("boom".into()))).expect("register");

  let payload = AdapterPayload::new(3_u32);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::Failure(AdapterError::Custom("boom".into())));
  assert!(leftover.is_none());
}
