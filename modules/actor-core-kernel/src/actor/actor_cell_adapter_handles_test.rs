use super::*;

#[test]
fn drop_adapter_refs_marks_lifecycle_stopped() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(system.clone(), Pid::new(50, 0), None, "adapter".to_string(), &props).expect("create actor cell");
  system.register_cell(cell.clone());

  let (_id, lifecycle) = cell.acquire_adapter_handle();
  assert!(lifecycle.is_alive());

  cell.drop_adapter_refs();
  assert!(!lifecycle.is_alive());
}

#[test]
fn remove_adapter_handle_stops_single_handle() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(system.clone(), Pid::new(51, 0), None, "adapter".to_string(), &props).expect("create actor cell");
  system.register_cell(cell.clone());

  let (id, lifecycle) = cell.acquire_adapter_handle();
  assert!(lifecycle.is_alive());

  cell.remove_adapter_handle(id);
  assert!(!lifecycle.is_alive());
}
