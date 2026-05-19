use fraktor_actor_core_kernel_rs::{actor::setup::ActorSystemConfig, system::ActorSystem};

fn main() {
  let _ = ActorSystem::create_started_from_config(ActorSystemConfig::default());
}
