use fraktor_actor_core_rs::core::kernel::routing::{RoundRobinRoutingLogic, RouterConfig};

fn main() {
  let _ = core::any::type_name::<Option<&'static dyn RouterConfig<Logic = RoundRobinRoutingLogic>>>();
}
