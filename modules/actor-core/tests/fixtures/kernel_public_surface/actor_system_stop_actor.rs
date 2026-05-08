use fraktor_actor_core_rs::core::kernel::{
  actor::Pid,
  system::ActorSystem,
};

fn main() {
  let system: &ActorSystem = todo!();
  let _ = system.stop_actor(Pid::new(1, 0));
}
