use fraktor_actor_core_kernel_rs::routing::{RoundRobinRoutingLogic, Router};

fn main() {
  let _ = Router::<RoundRobinRoutingLogic>::from_config;
}
