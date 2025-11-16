use alloc::string::ToString;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox,
  actor_prim::{Actor, ActorCellGeneric, ActorContextGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::Props,
  system::{ActorSystemGeneric, SystemState},
  typed::message_adapter::{AdapterFailure, AdapterOutcome, AdapterPayload, MessageAdapterRegistry},
};

struct Harness {
  system: ActorSystemGeneric<NoStdToolbox>,
  cell:   ArcShared<ActorCellGeneric<NoStdToolbox>>,
}

impl Harness {
  fn new() -> Self {
    let state = ArcShared::new(SystemState::new());
    let system = ActorSystemGeneric::from_state(state.clone());
    let props = Props::from_fn(|| ProbeActor);
    let pid = state.allocate_pid();
    let cell =
      ActorCellGeneric::create(state.clone(), pid, None, "adapter".to_string(), &props).expect("create actor cell");
    state.register_cell(cell.clone());
    Self { system, cell }
  }

  fn context(&self) -> ActorContextGeneric<'_, NoStdToolbox> {
    ActorContextGeneric::new(&self.system, self.cell.pid())
  }
}

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn registry_replaces_existing_adapter_for_same_type() {
  let harness = Harness::new();
  let mut registry = MessageAdapterRegistry::<i32, NoStdToolbox>::new();
  let ctx = harness.context();

  registry.register::<u32, _>(&ctx, |value| Ok(value as i32)).expect("first adapter");
  registry.register::<u32, _>(&ctx, |value| Ok((value as i32) * 2)).expect("second adapter");

  assert_eq!(registry.len(), 1);

  let payload = AdapterPayload::<NoStdToolbox>::new(5_u32);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::Converted(10));
  assert!(leftover.is_none());
}

#[test]
fn registry_returns_not_found_when_no_adapter_matches() {
  let registry = MessageAdapterRegistry::<i32, NoStdToolbox>::new();
  let payload = AdapterPayload::<NoStdToolbox>::new(1_u8);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::NotFound);
  assert!(leftover.is_some());
}

#[test]
fn registry_returns_failure_from_adapter() {
  let harness = Harness::new();
  let mut registry = MessageAdapterRegistry::<i32, NoStdToolbox>::new();
  let ctx = harness.context();
  registry.register::<u32, _>(&ctx, |_| Err(AdapterFailure::Custom("boom".into()))).expect("register");

  let payload = AdapterPayload::<NoStdToolbox>::new(3_u32);
  let (outcome, leftover) = registry.adapt(payload);
  assert_eq!(outcome, AdapterOutcome::Failure(AdapterFailure::Custom("boom".into())));
  assert!(leftover.is_none());
}
