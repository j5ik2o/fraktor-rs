use fraktor_actor_core_rs::routing::{RoundRobinRoutingLogic, Router};

fn main() {
  let _ = Router::<RoundRobinRoutingLogic>::from_config;
}
