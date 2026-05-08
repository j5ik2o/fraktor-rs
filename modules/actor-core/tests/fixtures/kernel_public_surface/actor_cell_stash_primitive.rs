use fraktor_actor_core_rs::core::kernel::actor::{
  ActorCell,
  error::ActorError,
  messaging::AnyMessage,
};

fn probe(cell: &ActorCell) -> Result<usize, ActorError> {
  cell.unstash_messages_with_limit(1, Ok::<AnyMessage, ActorError>)
}

fn main() {
  let _ = probe;
}
