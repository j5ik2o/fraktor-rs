use fraktor_actor_core_rs::core::kernel::routing::{Pool, RoundRobinRoutingLogic};

fn main() {
  let _ = core::any::type_name::<Pool<RoundRobinRoutingLogic>>();
}
