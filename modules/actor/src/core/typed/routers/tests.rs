use crate::core::typed::{routers::Routers, service_key::ServiceKey};

#[test]
fn group_returns_group_router_builder() {
  let key = ServiceKey::<u32>::new("test-group");
  let _builder = Routers::group(key);
}
