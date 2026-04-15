use fraktor_actor_core_rs::core::kernel::routing::{Group, RoundRobinRoutingLogic};

fn main() {
  let _ = core::any::type_name::<Group<RoundRobinRoutingLogic>>();
}
