use fraktor_actor_core_kernel_rs::system::{
  ActorSystem,
  state::{SystemStateShared, system_state::SystemState},
};

fn main() {
  let _ = ActorSystem::from_state(SystemStateShared::new(SystemState::new()));
}
