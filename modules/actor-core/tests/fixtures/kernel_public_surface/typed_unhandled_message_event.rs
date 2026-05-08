use fraktor_actor_core_rs::core::kernel::event::stream::TypedUnhandledMessageEvent;

fn main() {
  let _ = core::any::type_name::<TypedUnhandledMessageEvent>();
}
