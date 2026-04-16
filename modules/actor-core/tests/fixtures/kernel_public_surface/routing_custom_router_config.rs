use fraktor_actor_core_rs::core::kernel::routing::{CustomRouterConfig, RoundRobinRoutingLogic};

fn main() {
  let _ = core::any::type_name::<Option<&'static dyn CustomRouterConfig<Logic = RoundRobinRoutingLogic>>>();
}
