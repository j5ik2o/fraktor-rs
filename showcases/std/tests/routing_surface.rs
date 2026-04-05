use fraktor_actor_adaptor_rs::std::typed::Behaviors;
use fraktor_actor_rs::core::typed::{TypedProps, dsl::routing::Routers};

#[derive(Clone)]
struct Command(u32);

#[test]
fn routing_showcase_pool_router_builder_is_usable_without_build_step() {
  let sample = Command(1);
  assert_eq!(sample.0, 1);

  let _props = TypedProps::<Command>::from_behavior_factory(|| {
    Routers::pool::<Command, _>(2, || {
      Behaviors::receive_message(|_ctx, message: &Command| {
        let _value = message.0;
        Ok(Behaviors::same())
      })
    })
    .with_round_robin()
  });
}

#[test]
fn routing_showcase_example_does_not_suppress_print_stdout_lint() {
  let source = include_str!("../routing/main.rs");
  assert!(
    !source.contains("#[allow(clippy::print_stdout)]"),
    "routing showcase must not suppress clippy::print_stdout",
  );
}
