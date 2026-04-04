use crate::core::typed::{
  TypedProps,
  dsl::{
    Behaviors,
    routing::{GroupRouter, PoolRouter, Routers},
  },
  receptionist::ServiceKey,
};

#[test]
fn group_returns_group_router_surface() {
  let key = ServiceKey::<u32>::new("test-group");
  let _router: GroupRouter<u32> = Routers::group(key);
}

#[test]
fn group_router_builder_chain_starts_from_router_factory() {
  let key = ServiceKey::<u32>::new("test-group-public-chain");
  let _router: GroupRouter<u32> = Routers::group(key).with_random_routing(7);
}

#[test]
fn pool_returns_pool_router_surface() {
  let _router: PoolRouter<u32> = Routers::pool(3, Behaviors::ignore);
}

#[test]
fn pool_router_builder_chain_starts_from_router_factory() {
  let _router: PoolRouter<u32> = Routers::pool(3, Behaviors::ignore).with_random(7);
}

#[test]
fn typed_props_accepts_group_router_without_build_step() {
  let key = ServiceKey::<u32>::new("test-group-props");
  let _props = TypedProps::<u32>::from_behavior_factory(move || Routers::group(key.clone()).with_random_routing(7));
}

#[test]
fn typed_props_accepts_pool_router_without_build_step() {
  let _props = TypedProps::<u32>::from_behavior_factory(|| Routers::pool(3, Behaviors::ignore).with_random(7));
}
