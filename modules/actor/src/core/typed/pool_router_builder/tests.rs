use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::typed::{Behaviors, behavior::Behavior, routers::Routers};

#[test]
fn pool_router_builder_builds_behavior() {
  let builder = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore);
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
fn pool_router_builder_with_pool_size_override() {
  let builder = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_pool_size(5);
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
#[should_panic(expected = "pool size must be positive")]
fn pool_router_builder_rejects_zero_pool_size() {
  let _builder = Routers::pool::<u32, NoStdToolbox, _>(0, Behaviors::ignore);
}

#[test]
#[should_panic(expected = "pool size must be positive")]
fn pool_router_builder_with_pool_size_rejects_zero() {
  let _ = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_pool_size(0);
}
