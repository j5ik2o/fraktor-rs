use super::*;

#[test]
fn spawn_pipe_task_rejects_terminated_cell() {
  let actor_system = ActorSystem::new_empty();
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(913, 0), None, "pipe-stopped".to_string(), &props).expect("cell");
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  invoker.system_invoke(SystemMessage::Stop).expect("stop");

  let result = cell.spawn_pipe_task(Box::pin(async { Some(AnyMessage::new(1_i32)) }));

  assert!(matches!(result, Err(PipeSpawnError::TargetStopped)));
}
