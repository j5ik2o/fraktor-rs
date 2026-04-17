use fraktor_actor_core_rs::core::kernel::routing::{RoundRobinRoutingLogic, Router};

fn main() {
  let _ = Router::<RoundRobinRoutingLogic>::from_config;
}
